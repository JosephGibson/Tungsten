# 1.x backlog — draft

- **status:** draft
- **goal:** Keep everything the 1.0 plan defers in one place, with where it was deferred and what would bring it back, so that the must list can be cut without losing work.
- **non-goals:** Scoping or ordering 1.x work. DESIGN's non-commitments, which are not deferrals.
- **files to touch:** This file.
- **ordered steps:** Add a row whenever a workstream, a gate or the acceptance game cuts an item; review the list at the feature gate.
- **done-when:** At the 1.0 gate every row names its source; after 1.0 this file seeds the 1.x plans.

Sources are [criteria](criteria.md) sections unless another file is named.

| Item | Deferred in | Comes back when |
| --- | --- | --- |
| Editable text fields and IME | §4; [w01](w01-ui-text-suite.md) §11; the definition gate (Q8, `D-102`) | A game needs name entry |
| Accessibility (reduced motion among it, M56 Q5), docking, virtualization, localization, data-defined screens, an editable inspector, a command console | w01 §11, "Later" | Each gets its own plan |
| Owned glyph renderer (T2) and what needs it: per-glyph transforms and colour animation (wave, pop; the damage numbers' per-glyph version), in-shader outline, shadow and glow, bitmap fonts, premultiplied alpha, an initial atlas size, a readout of the atlas's bytes and growth frames (glyphon 0.12 keeps its sides private; M56 Q16 moved the known issue here), per-batch glyph release, text preparation off the main thread (W2 R3) | The glyph gate (`D-126`) | A game or the UI fixture needs one of those effects; T2 amends `D-026` |
| Integer scene scaling (`display.scale_mode`) | The glyph gate (`D-127`) | A pixel-art game asks; the UI stays at native resolution |
| JSON schemas for config files; live editing in the game | [w14](w14-tooling.md) | W1's widgets have shipped |
| Nested prefabs; patching live entities on hot reload | §8.7 | A game asks |
| Scene entries that instance prefabs | M51's non-goals (`D-157`) | A scene places an entity kind a prefab already describes, with overrides |
| Rotated and templated Tiled objects | M51 Q9 (`D-157`): a classed object with a rotation and any object with a `template` fail the load | A level needs turned props, or its author uses Tiled templates; rotation needs a pivot design for the default extract, which turns a sprite about its centre, and templates a `.tx` reader |
| A rotation or scale in prefab files; overrides of a prefab's sprite, tag and tween | M51 Q2, Q8 (`D-157`): a prefab spawns unrotated at scale 1, and an object's properties reach `components` only | A game places one kind at several sizes or angles, or a level changes a prefab's look per object |
| World snapshots and saves by reflection | [w11](w11-shipping-basics.md), §8.7 | Component reflection exists |
| Crash files for signals (segfault, driver abort) | [w11](w11-shipping-basics.md) | A game ships and needs them |
| Parallel system scheduler | [w02](w02-multi-core-rendering.md#decisions-the-pass-needs-first) | A CPU-bound game row needs it; Q6 set no bounds (`D-131`), so it is opt-in per system over `Sync` types |
| Fork-join default and tilemap extracts (R1) | The frame-loop gate (`D-131`), on the spike in `perf-runs/20261005-w2-r1-spike/` | C1 shows an extract-bound game row on a machine whose CPU has the memory bus to itself, or the bytes-per-instance work (C2) leaves an extract CPU-bound; it builds on `D-130`'s slice queries |
| Parallel custom extracts | [w02](w02-multi-core-rendering.md#options), R1 | A custom extract dominates a row |
| Pipelined render thread (R4) | The definition gate (Q4, `D-102`), as the [implementation plan](implementation-plan.md) §6 proposed | C1 shows a game-frame row over budget that R4 would fix |
| Parallel asset decode (R5) | [w02](w02-multi-core-rendering.md#options) | Startup metrics (proposal T2) ask for it |
| R2's later half: one command encoder per worker, submitted in order, and render bundles for static content such as tile layers; with them, bloom's offscreen passes ahead of the acquire when bloom ends a direct stack | [w02](w02-multi-core-rendering.md#follow-ups) follow-up 1; M44, whose split around the acquire records on one thread and keeps a bloom slot that writes the swapchain whole after it (Q13, `D-145`) | A worker mechanism exists (`D-131`) and a row is bound by command recording; render bundles need no worker, only a row that records unchanged content every frame |
| Kit items with no user by the freeze | §8.4 | A user appears |
| Slopes, one-way platforms and ladders for the kit's platformer controller | §10 Q22, M52 Q1 (`D-158`): the controller has a ground check, coyote time, a jump buffer and variable jump height | A game's level needs one; a slope needs a floor normal the controller follows, a one-way platform a filter by the side met |
| W6 features the acceptance game does not use | §3 | A game asks |
| Audio pause | The frame-loop gate (`D-132`): music plays through pause and level-up, effects are short | A game pauses with long effects playing |
| Deep-pile lever: `contact_hertz` 60 on the constant fixed step ([known issues](../../known-issues.md)) | [w03](w03-frame-loop.md#follow-ups) follow-up 1; M41 Q8 (`D-137`) | A game's pile needs the stiffer contacts; it moves the physics hash and every engine pile |
| Tile layers ordered among sprites by z in the default sprite channel, which draws every tilemap under every sprite | M42 Q2 (`D-138`) | A game needs a tile layer between its sprites without an extract of its own, as example 01 and the `integrated` bench row have; W16b's tilemap builder may take it |
| Game cursors (a sprite or a custom image), cursor visibility and grab | [w01](w01-ui-text-suite.md) §14, "Cursors"; M47's non-goals (`D-149`) | A game hides or replaces the cursor, or locks it for mouse-look; M47's system cursors go through the same bridge |
| Per-player action maps for local multiplayer | M54 Q5: actions read any pad, `InputState` keeps per-pad state | A game seats two players on one machine |
| Analog trigger values | M54 Q9: triggers are digital buttons | A game needs a throttle or a pressure-sensitive trigger |
| Rumble (force feedback) | M54 Q9: force feedback stays off, so its thread never starts | A game asks, with the thread rule amended for it |
| A pooled, allocation-free world-label push | M54 Q21 (`D-161`): `push` takes an owned `String` | Labels show in an allocation profile, with W1 M4's allocation-free values |
| Broader controls beyond checkboxes and sliders: progress bars, collapsible sections, tooltips, popups with click-outside dismissal, window resizing; with them the wheel, Home, End and the page keys on a slider and a stick's analog reach as its speed | The definition gate (Q8, `D-102`) and [w01](w01-ui-text-suite.md) §8 kept only checkboxes and sliders for 1.0; M55 (`D-162`) settled `D-149`'s popup and click-outside limits here | M5's debug views or a game need one; M5 keeps the drag threshold with its movable windows |
| Interaction-state easing: hover, press and focus looks blended over a theme duration | M56 Q5 (`D-163`): w01 §14's "may"; a game eases a hover colour with a colour `UiTrack` on `PointerEntered` and `PointerLeft` | A theme or a game asks for eased state looks across every widget |
| Size tracks: animating a widget's width or height through layout | M56 Q5 (`D-163`): the six track properties are paint-only and never relayout | A screen needs an expanding panel that pushes its siblings |
| Fading a text span with its own colour | M56 Q3 (`D-163`): a span's own colour stays baked into its glyphs, so opacity, a root's fade and colour tracks leave it opaque | A faded screen shows a multicoloured label |
| Inline icons in prompts: a pad glyph or key cap in place of the binding's name | M56 §2.11 (`D-164`): prompts are text through `Binding::label` | The acceptance game's prompts or a platform's guidelines need button glyphs |
| Keymap-aware key names in prompts | M56 §2.6 (`D-164`): labels follow the physical `KeyCode` in US layout, since winit 0.30 has no keymap query | winit offers a keymap query, or a player on another layout reports wrong prompts |
| Dragging a scroll panel's thumb, and horizontal scroll panels | M57 Q11 (`D-165`): scroll panels are vertical, moved by the wheel, by code and by focus scrolling into view | A list wider than its window, or a player who drags scrollbars |
| Saving which debug views are open | M57 Q5 (`D-166`): only places persist, since startup code decides which views open and a pixel fixture must never show one a developer left open | A developer asks to reopen their views across runs |
| Raising a window above its layer's others on click | M57 Q13 (`D-165`): views open at distinct places, roots keep their insertion order | Overlapping windows, the debug views' or a game's, need a front-to-back order the player picks |
