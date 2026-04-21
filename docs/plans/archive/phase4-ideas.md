---
status: archived
goal: Capture theoretical Phase 4 direction focused on demonstrating engine performance and rendering quality
non-goals: networking, 3D, scripting, WASM, full UI library, save/load, parallel scheduler, editor tooling
files-to-touch: none (ideas only; no implementation commitments)
---

# Phase 4 — Ideas Doc (Showcasing Performance + Quality)

Theoretical scoping notes. Archived on creation because it is a brainstorm, not an execution plan. If Phase 4 formally kicks off, a new top-level plan under `docs/plans/` supersedes this one.

## Frame

Phase 4 is not about adding capabilities — it is about *proving* the engine is worth using. A feature only counts if it produces a visible artifact: a demo scene, a gif, a benchmark number, a published page. Phase 3's tail (`M22` atlases, `M23` particles, `M24` tweens) is the foundation for this; do not skip it.

## Rendering Quality Track

- **Post-processing chain** — offscreen render target + composable passes (bloom, CRT/scanlines, vignette, chromatic aberration, color LUTs). Highest visual ROI; self-contained in `tungsten-render`.
- **2D lighting** — normal-mapped sprites + point/directional lights + soft shadows. Transforms look entirely. Start minimal.
- **MSDF text rendering** — crisp text at any zoom; addresses a perennial indie-engine weak spot.
- **Parallax + layered backgrounds** — cheap, huge perceived depth.
- **Screen transitions / shader flourishes** — fade, wipe, dissolve, glitch. Pairs with M20 scene system.

## Performance Demo Track

- **Sprite stress dialed up** — push [`examples/02_sprite_stress`](../../../examples/02_sprite_stress/) to 100k–500k; record numbers. Requires M22 atlases for an honest test.
- **Particle storm** — hundreds of thousands of GPU-instanced particles. Pairs with M23.
- **Physics pile** — thousands of dynamic bodies under the M11 broadphase.
- **Large tilemap scroll** — huge world with smooth traversal (M10 + M16 + atlases).
- **Published benchmark page** — `docs/perf/` graphs, machine specs, FPS-vs-entity-count curves. Optional comparisons with other Rust 2D engines.

## Showcase Scenes (the real deliverable)

Tech demos win over feature lists. Two or three polished scenes beat ten features.

- **Bullet hell / shmup** — natural exercise of sprite count, particles, lighting, tweens.
- **Pixel-art ARPG vertical slice** — one screen, beautifully lit, reusing platformer systems.
- **Procedural visualizer** — non-game scene (audio-reactive particles, life-at-scale, physics sandbox) for pure "look what it can do" appeal.

## Visibility Tooling

- Built-in screenshot/GIF/video capture on a key press (builds on M21 screenshot plumbing).
- Automated benchmark runs that output markdown tables committed to the repo.
- HUD polish (M18) so it presents well in recordings.

## Suggested Ordering

1. Finish Phase 3 tail cleanly — `M22`, `M23`, `M24`.
2. Post-processing chain — biggest visual delta per unit of work.
3. 2D lighting — start simple (point lights, no shadows).
4. One polished showcase scene (bullet hell or ARPG slice) exercising everything above.
5. Perf demos + published benchmark page.
6. Remaining showcase scenes + capture tooling.

## Deferred / Out of Scope

Carried from Phase 3 deferrals: change detection, full UI library, save/load, scripting, parallel scheduler. From DESIGN.md non-commitments: networking, editor tooling, asset preprocessing, 3D, WASM, config hot reload, skeletal animation, streaming assets, compressed textures, per-platform asset variants.

3D is a separate architectural conversation and not Phase 4 scope; see notes captured outside this doc.

## Trap to Avoid

"One more feature then I'll make the demo." The demo *is* the feature. If a Phase 4 item does not produce a gif-able scene or a benchmark number, cut it. Quality/performance showcases die from infinite polish on invisible plumbing.

## Guardrails Carried Forward

- No external ECS / game-engine crate (`D-005`).
- No async runtime.
- Any new dependency needs a `DECISIONS.md` entry under `D-015`.
- Don't regress the 2D architecture to anticipate 3D; lock 2D first.
