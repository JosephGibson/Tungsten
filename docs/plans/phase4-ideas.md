---
status: draft
goal: Scope Phase 4 as a renderer-focused phase (8 milestones) that opens the engine up to user-authored shaders, 2D lighting, parallax, instanced mesh particles, crisp MSDF text, and a polished collaborative showcase example — with every milestone producing a gif-able demo.
non-goals:
  - Not scoping everything at once; each milestone becomes its own `phase4-milestone-NN-*.md` plan.
  - No 3D. No scripting. No networking. No WASM. No editor. (Per `DESIGN.md § Non-Commitments`.)
  - No dedicated cleanup milestone — each feature milestone slices any monolith it touches.
  - Deferred lighting / shadow-casting occluders explicitly out of Phase 4.
  - Capture tooling (GIF/video/screenshot automation) dropped from Phase 4.
files to touch:
  - `docs/plans/phase4-ideas.md` (this file — planning only, no code)
  - downstream per-milestone plans in `docs/plans/phase4-milestone-NN-*.md`
ordered steps:
  1. Align on the final milestone list + ordering (open questions at bottom).
  2. Decide stock-effect roster for M26 (which ★-items ship, which become user-authored examples).
  3. Confirm consolidation: 6 milestones (recommended) vs 8 milestones (one-concept-per-milestone).
  4. Promote agreed milestones to individual `phase4-milestone-NN-*.md` plan files.
  5. Kick off M25 implementation.
done-when:
  - The user has signed off on the final milestone list + ordering.
  - Each selected milestone has a dedicated plan file with `status: draft`.
  - Stock-effect roster for M26 is locked (prevents M26 scope-creep).
---

## Guiding Principle (carried from the archived draft)

**A Phase 4 milestone only counts if it produces a gif-able demo or a benchmark number.** The trap is "one more feature then I'll make the demo." Every milestone below names its acceptance artifact so we don't slide into infinite plumbing.

## Context Digest

Tungsten is at workspace `v0.21.0`, branch `0.21`, with Phase 3 closed (`M12`–`M24`). The renderer currently supports:

- One pass, direct-to-swapchain, no depth buffer, no MSAA
- Three pipelines: [sprite](crates/tungsten-render/src/sprite.rs), [quad](crates/tungsten-render/src/quad.rs), [debug_line](crates/tungsten-render/src/debug_line.rs)
- Screen-space text via `glyphon` + `cosmic-text` ([D-026](DECISIONS.md))
- Sprite rotation + per-sprite tint + UV subrect (atlas-ready from M22)
- Screenshot capture via offscreen `RENDER_ATTACHMENT | COPY_SRC` texture
- Shaders are `include_str!`'d WGSL, NOT hot-reloadable ([D-023](DECISIONS.md))

Nothing today models materials, render targets, post-processing, lighting, normal maps, depth, MSAA, per-layer camera scroll, instanced non-quad geometry, user-authored shaders, or scalable text.

## Proposed Shape — 8 Milestones

User called 7 in the last round, but separately granted bloom its own milestone AND starred 20 stock effects. Three-way conflict: bloom-standalone + full starred roster + other six concepts = 8 minimum. This draft settles on 8. If we want 7, the honest cut is either (a) drop bloom back into M26, or (b) demote ~8 starred effects to "user-authored examples after Phase 4."

### M25 — Render Foundation (offscreen target + depth + shader hot reload)

**Why first:** everything else needs a separate scene render target AND fast shader iteration. Depth buffer folded in since M27 lighting wants it for correct overlap, and it's cheap to add while we're already rewiring the main pass.

Scope sketch:

- Minimal "named render targets + ordered passes" abstraction (NOT a DAG render graph).
- Depth buffer on the scene target; **optional depth-testing path** for sprites so overlap can be depth-resolved instead of CPU-sorted. Keep `z_order` CPU-sort as the default to avoid regressing existing examples.
- Optional MSAA (configurable via `tungsten.json`, off by default).
- Shaders become manifest-tracked assets; `notify` watcher re-validates via `naga` on change; on failure keep the previous module and log `error`. New `DECISIONS.md` entry reverses / narrows [D-023](DECISIONS.md).
- Cracks `renderer.rs` (~31KB) into `passes/`, `targets.rs`, `surface.rs`, `timing.rs`.

**Acceptance artifact:** gif of `cargo run -p example-02-sprite-stress` going through the offscreen pipeline; editing `sprite.wgsl` and seeing the visual update without a rebuild.

### M26 — Materials + Post-Processing Stack + Tween→Material Bridge

**Why second:** M25 gives us the target + hot reload. Now the user gets to author shaders and drive their uniforms from tweens for damage flashes, charge-up glows, etc.

Scope sketch:

- `Material` asset type: manifest entry references a `.wgsl` + fixed uniform schema (no reflection — uniform struct is declared Rust-side, mirrored in WGSL).
- Sprite render path can opt into a non-default material; default stays built-in `sprite.wgsl`.
- Screen-space post stack: ordered list of full-screen passes reading `scene_color` → intermediate → swapchain.
- **Tween→Material uniform bridge**: a `TweenChannel` variant that writes into a named material uniform slot. Unlocks damage flashes, pickup pulses, charge effects. Tiny surface-area addition to M24.
- **Tungsten Stock Effects library** — all starred non-bloom effects in §Stock Effect Roster (~19 effects including the lit add-ons that ship later behind M28). Bloom is carved out to M27 as its own milestone.

**Note:** `04_shader_playground` for this milestone is deliberately **minimal** — bouncing sprite + effect toggle overlay, not a game scene. Heavy example work is reserved for M32 capstone.

**Acceptance artifact:** minimal `04_shader_playground` cycling through stock effects; separate clip showing a tween-driven damage flash on the platformer character.

### M27 — Bloom

**Why third (between materials and lighting):** bloom is a multi-pass downsample-blur-composite operation significantly heavier than anything in the M26 stock roster. Breaking it out keeps M26 tight and gives bloom the attention it needs (mip chain allocation, Karis average, threshold + knee, final additive composite).

Scope sketch:

- Mip-chained scene_color (~6 levels) allocated in M25's target system.
- Threshold extract pass → progressive downsample with soft knee → upsample/blur with tent filter → additive composite into final post-stack slot.
- Exposed as a single `Bloom { threshold, intensity, radius }` post-stack entry.
- Hot reload of the bloom shaders works through M25's pipeline.

**Acceptance artifact:** gif of a scene with emissive sprites (placeholder — real lit scene lands in M28) toggling bloom on/off.

### M28 — 2D Lighting (Forward, Normal-Mapped)

**Why fourth:** depends on M26 materials (the lit sprite path IS a material) and M25 target. M27 bloom pairs beautifully with emissive-mask lighting but isn't a hard dependency either direction.

Scope sketch:

- Sprite manifest gains optional `normal_map: AssetId` field.
- `Light` components: `Point { radius, falloff }`, `Directional { angle }`, shared `color`/`intensity`.
- Extract builds `LightUBO` (cap N=16 visible lights/frame; cull by view AABB).
- Lit fragment shader samples normal, does N-dot-L per light, accumulates, multiplies albedo × tint × lighting.
- `AmbientLight` resource (defaults to `Vec3::ONE` so unlit scenes don't change).
- Ships the three **lit-sprite add-ons** from the roster: ★ emissive mask, ★ rim lighting. (Specular highlight remains unstarred / deferred.)

Explicit non-goals: deferred rendering, shadow casters, occluder polygons, volumetric lights, GI. Those are Phase 5.

**Acceptance artifact:** platformer gets a normal-mapped character + two colored point lights + one directional; gif shows lighting response as the player moves, plus emissive eyes that bloom (integrates with M27).

### M29 — Parallax Layers + Game-Feel Polish (shake + squash/stretch)

**Why fifth:** cheapest, most decoupled, big visual bang-per-buck. Bundles the small game-feel wins because they're all "camera/transform helpers" and none alone justifies its own milestone.

Scope sketch:

- `ParallaxLayer { scroll_factor: Vec2, depth_bucket: i32 }` component, independent of `Sprite.z_order`.
- Extract groups sprites by `depth_bucket`; each bucket gets its own view_proj derived from `CameraState.position * scroll_factor`.
- **Camera screen-shake**: trauma-model shake on `CameraState` with decay, driven by an event or tween.
- **Sprite squash/stretch helper**: a `SpriteSquashStretch` component + tiny system that tweens non-uniform `Transform.scale` on trigger (land, hit, pickup). Sits atop M24 tweens; no new tween surface.

**Acceptance artifact:** platformer scrolls with 3-layer parallax; character lands with a visible squash; on hit, screen shakes + damage-flash material uniform fires.

### M30 — Instanced Mesh Particles + Screen Transitions

**Why sixth:** bundles "new draw paths that use M26's material system." Instanced meshes unlock trails/beams/ribbons; transitions are a materials-post-stack use case. Both decoupled from lighting, both small, both motivate the material system further.

Scope sketch:

- **Instanced mesh particles**: a second particle render path alongside the single-quad default. User supplies a small mesh (line segment, triangle, custom); emitter picks the path. Could eventually replace `debug_line`, but that's not a Phase 4 goal.
- **Screen transitions**: `Transition` resource lets scene code request "push state `X` with `RadialWipe { duration: 0.6 }`" — engine runs out-shader on source, swaps state, runs in-shader on target. Reuses M26 post-stack and the fade/wipe/dissolve stock effects.

**Acceptance artifact:** trails behind a bullet-hell-style spawn pattern (instanced meshes); `03_scene_state` transitioning with each built-in wipe.

### M31 — MSDF Text Rendering (final graphics-track milestone before capstone)

**Why seventh (positioned deliberately before M32 capstone):** user wants text polished right before the showcase example so title cards, HUD, and world labels are pixel-perfect in the capstone. No other milestone depends on MSDF, and the [D-026](DECISIONS.md) narrow-vs-replace conversation is cleaner with everything else shipped.

Scope sketch:

- Offline bake: `msdfgen` Rust crate + `ttf_parser` build a per-font `.msdf.png` + `.msdf.json` (glyph UV/metrics) at startup. Session-cached; not a manifest-pipeline step yet.
- New text pipeline `msdf_text.wgsl` — ~20-line fragment shader doing median-of-RGB distance thresholding.
- `cosmic-text` stays for layout (shaping, BiDi, line breaking); only the rasterizer swaps. [D-026](DECISIONS.md) gets narrowed via a new decision entry, not reversed.
- New `MsdfText` component coexisting with existing glyphon-backed `TextSection`. Opt-in per call-site; default stays glyphon.
- Free outlines/glows/shadows as shader-threshold operations — huge ergonomic win.

**Acceptance artifact:** side-by-side gif — same text at 3 zoom levels using glyphon vs MSDF. Bonus: a shader toggle that animates an outline + glow on MSDF text.

### M32 — Collaborative Showcase Example (Phase 4 capstone — SPECULATIVE STUB)

**Status for now:** **stub only**. User has explicitly deferred design until after all shader/graphics systems ship. Do not pre-commit scope here beyond the stub.

**Why last:** every prior Phase 4 milestone has been infrastructure plus isolated demos. M32 is where the user and assistant sit down together and build ONE complex example that exercises materials, bloom, lighting, parallax, particles, transitions, and MSDF text.

Speculative scope (do not lock until kickoff):

- Shape picked at M32 kickoff, NOT now. Candidate seeds carried forward from the archived draft purely for reference: bullet-hell shmup, top-down ARPG vertical slice, atmospheric exploration scene, procedural audio-reactive visualizer.
- Must exercise: M26 materials + stock effects, M27 bloom, M28 lighting (normal-mapped + emissive), M29 parallax + shake + squash, M30 instanced particles + at least one transition, M31 MSDF title card.
- Target lives in `examples/05_showcase/` (name TBD) with its own `assets/` manifest.
- Acceptance by demo: a ~30-second clip that would be convincing as a "this is what Tungsten can do" showreel.

**Acceptance artifact:** the clip itself + PNG stills committed under `docs/showcase/`.

**Explicit rule:** do not open `examples/05_showcase/` or start asset production during M25–M31. Use the minimal `04_shader_playground` scene inside each milestone for verification.

## On MSDF — What It Is, Why We're Keeping It

**What it is:** MSDF = Multi-channel Signed Distance Field. Each glyph bakes ONCE into a texture where pixels encode distance to the nearest edge in R/G/B separately. A ~20-line fragment shader reconstructs crisp edges at any zoom via RGB-median threshold.

**Why it looks better than `glyphon`'s per-size raster:**

- Crisp at any zoom — 32px atlas glyph renders sharp at 12px or 512px with no re-rasterization.
- Free outlines, glows, shadows, bevel effects — threshold operations in shader.
- Tiny memory — one 512×512 atlas per font covers all sizes; glyphon keeps cached rasters per-size.

**What we're committing to for M30:** narrow [D-026](DECISIONS.md) (don't reverse) — keep `cosmic-text` for layout (shaping, BiDi), swap only the rasterizer for opted-in text. Bake at startup via `msdfgen` crate; no asset-preprocessing pipeline step yet. Coexists with glyphon; glyphon stays the default.

## External Shader Sources — Dev Time Reality Check

You asked about LYGIA dev-time overhead vs alternatives. Concrete answer:

**[LYGIA](https://lygia.xyz/) cherry-pick + vendor** (my recommendation):

- **What it gives us:** noise, hashes, color-space conversions, easing curves, distance functions, common math primitives. The fiddly parts of writing effects from scratch.
- **What it does NOT give us:** complete effects. LYGIA is a function library, not an effect library. Each stock effect we ship is still a ~30–80 line hand-authored composition using LYGIA primitives.
- **WGSL maturity:** LYGIA's WGSL branch covers a good portion of their primitives but not 100%. Some files we want may still be GLSL-only and need manual translation (30 min – 2 h per primitive).
- **Dev-time savings vs pure from-scratch:** roughly 20–30% — we save on "scratch math" but most authoring time is effect composition, tuning, and visual iteration.
- **Runtime cost:** zero. We vendor MIT-licensed text files into `tungsten-render/shaders/stock/lygia/` with a header attribution. No crate dependency, no [D-015](DECISIONS.md) conversation.
- **Licensing:** LYGIA is MIT-compatible.

**Alternatives ranked by dev-time cost:**

1. **LYGIA cherry-pick (recommended):** ~same or slightly faster than from-scratch; lowest risk.
2. **Pure from-scratch:** tailored exactly to our needs; +10–20% authoring time; no external dependency or attribution.
3. **Port Godot shaders (GLSL):** high translation cost; excellent design inspiration; use as reference only.
4. **Port Open-Shaders (GLSL/HLSL):** same story as Godot.

**Honest assessment:** LYGIA helps, but don't expect a drop-in. Treat it as "save time on boring math helpers; still hand-author the effects."

## Stock Effect Roster (for M26)

**Priority key:** ★ = picked for M26 initial roster, open for promotion/demotion per your input.

**Color / Tone:**

- ★ Tonemap (Reinhard or ACES-approximation)
- ★ Vignette
- ★ Color grading via 3D LUT (`.cube` or baked PNG LUT strip)
- ★ Chromatic aberration
- ★ Hue shift / saturation / contrast
- ★ Sepia, monochrome, duotone

**Retro / Stylized:**

- ★ CRT (barrel distortion + scanlines + phosphor)
- ★ Film grain (animated noise)
- ★ Dithering (Bayer / Floyd-Steinberg approx)
- Palette quantize / palette swap
- 1-bit / low-bit posterize
- ★ Pixel-art outline (8-directional threshold)

**Motion / Transition (used by M30):**

- ★ Fade (plain alpha crossfade)
- ★ Radial wipe
- ★ Dissolve (threshold against noise)
- Directional motion blur
- ★ Glitch (RGB-split + slice displacement)
- ★ Pixelate-out

**Environmental:**

- Water ripple (animated sine displacement + edge foam)
- Heat distortion (2D normal sample → UV offset)
- ★ Fog / vignette fog
- ★ Screen-space god rays (radial blur from light position)
- ★ Bloom (downsample + blur + additive) — **carved out to M27 as its own milestone**

**Lit-sprite add-ons (ship with M28 lighting):**

- ★ Emissive mask
- ★ Rim lighting
- Specular highlight on normal-mapped sprites

## Other Graphics Ideas — Status

From my earlier side-ideas list:

1. ✅ **Tween→Material uniform bridge** — folded into M26.
2. ✅ **Sprite squash/stretch** — folded into M29.
3. ✅ **Camera screen-shake** — folded into M29 alongside squash/stretch ("wherever makes sense" per user).
4. ✅ **Depth-buffer sprite sorting** — folded into M25 (optional; CPU `z_order` stays default).
5. ✅ **Instanced particle-mesh path** — folded into M30.
6. ⏸️ **Async texture upload / streaming** — not folded in; flagged for Phase 5.

## Structural Weak Spots (Flagged, Not Scoped as Milestones)

Each feature milestone slices the monolith it touches:

- **`app.rs` (~48KB)** — M25 touches render parts of the frame path; natural crack point.
- **`asset_loader.rs` (~34KB)** — M26 adds `Material`, M28 adds normal-map channel, M31 adds MSDF atlas. All good opportunities to split `asset_loader/` into per-type modules.
- **`renderer.rs` (~31KB)** — M25 mandates cracking this open.
- **Physics semi-fixed timestep** — [D-033](DECISIONS.md) flags this as preferred upgrade. Phase 5 candidate unless we trip on it.

## Final Ordering

1. **M25** — Render foundation (offscreen + depth + shader hot reload)
2. **M26** — Materials + post stack + tween→material bridge + non-bloom stock effects (~19 effects)
3. **M27** — Bloom (mip-chain, threshold, knee, additive composite)
4. **M28** — 2D lighting (forward, normal-mapped) + emissive mask + rim lighting
5. **M29** — Parallax + game-feel polish (screen-shake + squash/stretch)
6. **M30** — Instanced mesh particles + screen transitions
7. **M31** — MSDF text (positioned just before capstone at user's request)
8. **M32** — Collaborative showcase example (SPECULATIVE STUB until M31 closes)

Hard dependencies: M25 → M26 → M27 → M28. M29, M30 can swap. M31 must precede M32. M32 must not start until M31 lands.

## Resolved Decisions From Planning

- **Milestone count:** 8 (locked, with note that the "7" answer collided with bloom-as-own-milestone + 20 starred effects).
- **Stock effect roster:** all starred effects in §Stock Effect Roster ship across M26 (non-bloom), M27 (bloom), M28 (emissive + rim). ~22 effects total in Phase 4.
- **Bloom:** own milestone (M27).
- **Camera screen-shake:** folded into M29 with squash/stretch.
- **M32 showcase:** speculative stub; not designed until M31 closes. No asset production or scope lock before then.
- **Ordering:** MSDF at M31 right before capstone per user preference.
- **M26 playground:** minimal scene only (bouncing sprite + effect toggle), no game-scene authoring until M32.
- **Shader source strategy:** vendor LYGIA WGSL snippets with attribution into `tungsten-render/shaders/stock/lygia/`; hand-author the effect compositions on top.

## Open Items for Per-Milestone Planning

These get resolved in each milestone's dedicated plan file, not here:

- Exact material uniform schema (M26).
- Bloom parameters surface and mip-count tuning (M27).
- `LightUBO` layout + packing + light-cull algorithm (M28).
- `ParallaxLayer` interaction with existing tilemap and particle extracts (M29).
- Instanced mesh format (M30).
- MSDF atlas bake location and asset caching (M31).

## Sources

- [Chlumsky/msdfgen — MSDF generator](https://github.com/Chlumsky/msdfgen)
- [Blatko1/awesome-msdf](https://github.com/Blatko1/awesome-msdf)
- [Signed Distance Field Fonts — redblobgames](https://www.redblobgames.com/blog/2024-03-21-sdf-fonts/)
- [msdfgen Rust crate](https://docs.rs/msdfgen)
- [LYGIA Shader Library](https://lygia.xyz/)
- [Open-Shaders collection](https://github.com/repalash/Open-Shaders)
- [Godot Shaders community library](https://godotshaders.com/)
- [gdquest-demos/godot-shaders (2D shader collection)](https://github.com/gdquest-demos/godot-shaders)
- [Godot CanvasItem shader reference](https://docs.godotengine.org/en/stable/tutorials/shaders/shader_reference/canvas_item_shader.html)
- [LearnOpenGL — 2D Post-Processing](https://learnopengl.com/In-Practice/2D-Game/Postprocessing)
- [fontdue — pure-Rust font rasterizer](https://github.com/mooman219/fontdue)
