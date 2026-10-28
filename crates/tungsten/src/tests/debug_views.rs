use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tungsten_core::assets::{AssetRegistry, FilterMode, TextureHandle, UvRect};
use tungsten_core::components::{Sprite, Transform, Visibility};
use tungsten_core::input::{InputState, KeyCode, MouseButton};
use tungsten_core::text::{EllipsisAt, TextAlign, TextOverflow, TextWrap};
use tungsten_core::ui::{PaintCommand, Rect, Theme, WidgetState};
use tungsten_core::{Config, Entity, UserFolder};

use super::*;
use crate::plugins::{DefaultPlugins, UiPlugin};
use crate::testing::UiHarness;
use crate::{App, DebugHud, InspectorState, SystemTimingOverlay};

fn app() -> App {
    App::new(Config::default()).expect("App::new failed")
}

fn tempdir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tungsten_debug_views_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An app whose user folder is `dir`.
fn app_at(dir: &Path) -> App {
    let mut config = Config::default();
    config.game.user_folder = UserFolder::At(dir.to_path_buf());
    App::new(config).unwrap()
}

/// Presses and releases `key`, a frame each.
fn tap(harness: &mut UiHarness, key: KeyCode) {
    harness.key_down(key);
    harness.step(1);
    harness.key_up(key);
    harness.step(1);
}

/// The three views' roots.
fn roots(harness: &UiHarness) -> [Option<WidgetId>; 3] {
    let world = harness.world();
    [
        world.resource::<DebugHud>().window(),
        world.resource::<SystemTimingOverlay>().window(),
        world.resource::<InspectorState>().window(),
    ]
}

/// Every node under `root`, itself included.
fn subtree(tree: &UiTree, root: WidgetId) -> Vec<WidgetId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(id);
        stack.extend_from_slice(tree.children(id));
    }
    out
}

fn open_all(harness: &mut UiHarness) {
    let world = harness.world_mut();
    world.resource_mut::<DebugHud>().enabled = true;
    world.resource_mut::<SystemTimingOverlay>().enabled = true;
    world.resource_mut::<InspectorState>().enabled = true;
}

#[test]
fn f4_f2_and_f3_open_and_close_their_windows() {
    let mut harness = UiHarness::new(app());
    assert_eq!(roots(&harness), [None, None, None], "all off by default");
    for key in [KeyCode::F4, KeyCode::F2, KeyCode::F3] {
        tap(&mut harness, key);
    }
    let open = roots(&harness);
    let tree = harness.tree();
    for (root, title) in open.iter().zip(["HUD", "Systems", "Inspector"]) {
        let root = root.expect("open");
        assert_eq!(tree.layer(root), Some(UiLayer::Debug));
        assert_eq!(tree.accessible_name(root), Some(title));
        assert!(
            tree.drag_handle(tree.children(root)[0]),
            "the title bar drags"
        );
        assert!(tree.rect(root).is_some(), "laid out");
    }
    for key in [KeyCode::F4, KeyCode::F2, KeyCode::F3] {
        tap(&mut harness, key);
    }
    assert_eq!(roots(&harness), [None, None, None]);
    for root in open {
        assert!(!harness.tree().contains(root.unwrap()), "removed");
    }
    // A root a game removed is built again.
    tap(&mut harness, KeyCode::F4);
    let hud = roots(&harness)[0].unwrap();
    harness.tree_mut().remove(hud);
    harness.step(1);
    let again = roots(&harness)[0].unwrap();
    assert_ne!(again, hud);
    assert!(harness.tree().contains(again));
}

/// A theme whose panels' and labels' looks, and their focused, hovered and
/// pressed variants, change every property a view could inherit; panel
/// text transparent.
fn hostile_theme() -> Theme {
    let mut theme = Theme::default();
    let text = |color: [u8; 4]| TextOverrides {
        family: Some("sans".into()),
        size: Some(40.0),
        line_height: Some(50.0),
        weight: Some(900),
        italic: Some(true),
        color: Some(color),
        align: Some(TextAlign::Center),
        wrap: Some(TextWrap::WordOrGlyph),
        overflow: Some(TextOverflow::Ellipsis {
            at: EllipsisAt::End,
            lines: Some(1),
        }),
        letter_spacing: Some(0.5),
    };
    let variant = |color: [u8; 4]| StyleOverrides {
        background: Some([255, 0, 0, 255]),
        border: Some([0, 255, 0, 255]),
        border_width: Some(7.0),
        corner_radius: Some(9.0),
        text: text(color),
    };
    for (kind, color) in [
        (&mut theme.kinds.panel, [0, 0, 0, 0]),
        (&mut theme.kinds.label, [255, 0, 255, 255]),
    ] {
        kind.normal.background = [255, 0, 0, 255];
        kind.normal.border = [0, 255, 0, 255];
        kind.normal.border_width = 7.0;
        kind.normal.corner_radius = 9.0;
        kind.normal.text = text(color);
        kind.focused = variant(color);
        kind.hovered = variant(color);
        kind.pressed = variant(color);
    }
    theme
}

/// The palette's text colours (M58).
const TEXT_COLORS: [[u8; 4]; 13] = [
    VALUE, LABEL, GREEN, YELLOW, ORANGE, RED, AMBER, CYAN, VIOLET, BLUE, MAGENTA, ERROR_TEXT,
    WARN_TEXT,
];

/// Whether every node under `root` resolved to the view's look, not the
/// theme's (M58): the engine font at the body's 15 on 20 or a title's 17
/// on 22, at weight 400 or 600, in a palette colour, unspaced, wrapped by
/// glyph to one line that ends in `…`, and no hostile fill or border.
fn assert_own_look(tree: &UiTree, root: WidgetId) {
    for id in subtree(tree, root) {
        let style = tree.resolved_style(id).expect("resolved");
        let what = format!("{id:?} {:?}", tree.kind(id));
        assert_ne!(style.background, [255, 0, 0, 255], "{what}");
        assert_ne!(style.border, [0, 255, 0, 255], "{what}");
        assert_ne!(style.border_width, 7.0, "{what}");
        assert_ne!(style.corner_radius, 9.0, "{what}");
        let text = &style.text;
        assert_eq!(text.family, crate::ENGINE_FONT_ID, "{what}");
        assert!(
            matches!((text.size, text.line_height), (15.0, 20.0) | (17.0, 22.0)),
            "{what}: {} on {}",
            text.size,
            text.line_height
        );
        assert!(matches!(text.weight, 400 | 600), "{what}: {}", text.weight);
        assert!(!text.italic, "{what}");
        assert!(
            TEXT_COLORS.contains(&text.color),
            "{what}: {:?}",
            text.color
        );
        assert!(
            matches!(
                text.layout.align,
                TextAlign::Start | TextAlign::End | TextAlign::Center
            ),
            "{what}"
        );
        assert_eq!(text.layout.wrap, TextWrap::Glyph, "{what}");
        assert_eq!(
            text.layout.overflow,
            TextOverflow::Ellipsis {
                at: EllipsisAt::End,
                lines: Some(1),
            },
            "{what}"
        );
        assert_eq!(
            text.layout.letter_spacing, 0.0,
            "{what}: the theme's 0.5 em"
        );
    }
}

#[test]
fn every_node_a_view_builds_keeps_its_look_under_a_game_theme() {
    let mut harness = UiHarness::new(app());
    harness.tree_mut().set_theme(hostile_theme());
    open_all(&mut harness);
    {
        let world = harness.world_mut();
        world.resource_mut::<LogConsole>().enabled = true;
        world.resource_mut::<UiInspector>().enabled = true;
    }
    harness.step(3);
    let [hud, overlay, inspector] = roots(&harness).map(Option::unwrap);
    let console = harness.world().resource::<LogConsole>().window().unwrap();
    let ui = harness.world().resource::<UiInspector>().window().unwrap();
    let tree = harness.tree();
    for root in [hud, overlay, inspector, console, ui] {
        assert_own_look(tree, root);
    }
    let weights: Vec<u16> = subtree(tree, hud)
        .iter()
        .map(|id| tree.resolved_style(*id).unwrap().text.weight)
        .collect();
    assert!(weights.contains(&400) && weights.contains(&600));
    assert_eq!(tree.resolved_style(hud).unwrap().background, PANEL);
    assert_eq!(tree.resolved_style(hud).unwrap().border, BORDER);
    assert_eq!(
        tree.resolved_style(hud).unwrap().border_width,
        1.0,
        "1 px at scale 1"
    );
    let title = tree.children(hud)[0];
    let caption = tree.children(title)[0];
    let caption_text = &tree.resolved_style(caption).unwrap().text;
    assert_eq!(
        (
            caption_text.size,
            caption_text.line_height,
            caption_text.weight
        ),
        (17.0, 22.0, 600),
        "the title"
    );

    // Hovered and pressed, the title bar keeps it too.
    let title = tree.children(overlay)[0];
    let at = tree.rect(title).unwrap();
    let centre = (at.min + at.max) * 0.5;
    harness.pointer_to(centre.x, centre.y);
    harness.pointer_down();
    harness.step(1);
    assert_eq!(
        harness.tree().widget_state(title),
        Some(WidgetState::Pressed)
    );
    assert_own_look(harness.tree(), overlay);
    harness.pointer_up();
    harness.step(1);

    // The overlay's body scrolls, and its thumb takes the view's colour at
    // half its alpha, never the theme's transparent panel text.
    let body = harness.tree().children(overlay)[1];
    let max = harness.tree().scroll_max(body).unwrap();
    assert!(max > 0.0, "the schedule's systems overflow 45% of 720");
    assert!(harness.tree_mut().set_scroll(body, 40.0));
    let list = harness.paint();
    let thumbs: Vec<&PaintCommand> = list
        .commands
        .iter()
        .filter(|command| {
            matches!(command, PaintCommand::Rect { fill: [232, 232, 236, 128], rect, .. } if rect.width() == 4.0)
        })
        .collect();
    assert_eq!(thumbs.len(), 1, "the overlay's: {thumbs:?}");
}

#[test]
fn views_dock_down_one_corner_and_a_saved_place_wins() {
    let mut harness = UiHarness::new(app());
    {
        let world = harness.world_mut();
        world.resource_mut::<DebugHud>().corner = HudCorner::TopRight;
        world.resource_mut::<SystemTimingOverlay>().corner = HudCorner::TopRight;
        world.resource_mut::<InspectorState>().corner = HudCorner::TopRight;
    }
    open_all(&mut harness);
    harness.step(4);
    let [hud, overlay, inspector] = roots(&harness).map(Option::unwrap);
    let tree = harness.tree();
    let rect = |id| tree.rect(id).unwrap();
    assert_eq!(rect(hud).min.y, 8.0, "the HUD's padding");
    assert_eq!(rect(hud).max.x, 1272.0);
    assert_eq!(rect(overlay).min.y, rect(hud).max.y + 8.0, "below the HUD");
    assert_eq!(rect(overlay).max.x, 1272.0);
    assert_eq!(rect(inspector).min.y, rect(overlay).max.y + 8.0);

    // A saved place wins, and the next view docks as if it were not there.
    let mut harness = UiHarness::new(app());
    {
        let world = harness.world_mut();
        world.resource_mut::<DebugHud>().corner = HudCorner::TopRight;
        world.resource_mut::<SystemTimingOverlay>().corner = HudCorner::TopRight;
        world.resource_mut::<InspectorState>().corner = HudCorner::TopRight;
        world
            .resource_mut::<WindowPlacements>()
            .set("systems", Vec2::new(50.0, 60.0));
    }
    open_all(&mut harness);
    harness.step(4);
    let [hud, overlay, inspector] = roots(&harness).map(Option::unwrap);
    let tree = harness.tree();
    assert_eq!(tree.window_position(overlay), Some(Vec2::new(50.0, 60.0)));
    assert_eq!(
        tree.rect(inspector).unwrap().min.y,
        tree.rect(hud).unwrap().max.y + 8.0,
        "directly below the HUD"
    );
}

/// Where `id` paints, in window pixels: its visual rect at the scale
/// factor, which a debug root's units span with the debug scale (M58).
fn physical(harness: &UiHarness, id: WidgetId) -> Rect {
    let scale = harness.world().resource::<ScaleFactor>().get();
    let rect = harness.tree().visual_rect(id).unwrap();
    Rect::new(rect.min * scale, rect.max * scale)
}

/// Drags `root` by its title bar's caption by `by`, in window pixels.
fn drag(harness: &mut UiHarness, root: WidgetId, by: Vec2) {
    let title = harness.tree().children(root)[0];
    let caption = harness.tree().children(title)[0];
    let rect = physical(harness, caption);
    let from = (rect.min + rect.max) * 0.5;
    harness.pointer_to(from.x, from.y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(from.x + by.x, from.y + by.y);
    harness.step(1);
    harness.pointer_up();
    harness.step(2);
}

/// Opens a narrow HUD: small text keeps its long rows inside 1280.
fn open_narrow_hud(harness: &mut UiHarness) {
    let hud = &mut *harness.world_mut().resource_mut::<DebugHud>();
    hud.enabled = true;
    hud.font_size = 10.0;
    hud.line_height = 12.0;
}

#[test]
fn a_drag_saves_the_place_in_the_user_folder_and_the_window_reopens_there() {
    let dir = tempdir();
    let mut harness = UiHarness::new(app_at(&dir));
    open_narrow_hud(&mut harness);
    harness.step(2);
    let hud = roots(&harness)[0].unwrap();
    let start = harness.tree().window_position(hud).unwrap();
    drag(&mut harness, hud, Vec2::new(200.0, 100.0));
    let moved = start + Vec2::new(200.0, 100.0);
    assert_eq!(harness.tree().window_position(hud), Some(moved));
    let file = dir.join("settings").join(DEBUG_WINDOWS_FILE);
    assert_eq!(
        WindowPlacements::load(&file).get("hud"),
        Some(moved),
        "saved"
    );
    harness.step(3);
    assert_eq!(
        harness.tree().window_position(hud),
        Some(moved),
        "a saved window leaves its corner"
    );

    let mut reopened = UiHarness::new(app_at(&dir));
    open_narrow_hud(&mut reopened);
    reopened.step(3);
    let hud = roots(&reopened)[0].unwrap();
    assert_eq!(reopened.tree().window_position(hud), Some(moved));
}

#[test]
fn a_place_saved_on_a_larger_screen_reopens_inside_a_smaller_one() {
    let mut harness = UiHarness::new(app());
    {
        let world = harness.world_mut();
        *world.resource_mut::<WindowSize>() = WindowSize {
            width: 800,
            height: 600,
        };
        world
            .resource_mut::<WindowPlacements>()
            .set("hud", Vec2::new(1800.0, 900.0));
        world.resource_mut::<DebugHud>().enabled = true;
    }
    // A saved window clamps with the last layout's size: the HUD widens as
    // its statistics arrive, and the clamp follows a frame later.
    harness.step(4);
    let hud = roots(&harness)[0].unwrap();
    let rect = harness.tree().rect(hud).unwrap();
    // 800×600 steps to 3/4: the debug viewport is 1066.7×800 units.
    let viewport = harness.tree().layer_viewport(UiLayer::Debug).unwrap();
    assert!((viewport - Vec2::new(800.0, 600.0) / 0.75).length() < 1e-3);
    assert!(
        rect.min.x >= 0.0
            && rect.min.y >= 0.0
            && rect.max.x <= viewport.x + 1e-3
            && rect.max.y <= viewport.y + 1e-3,
        "{rect:?}"
    );
    assert!(
        (rect.max.x - (viewport.x - 8.0)).abs() < 1e-3
            || (rect.max.y - (viewport.y - 8.0)).abs() < 1e-3,
        "clamped to the edge, then snapped onto the margin line: {rect:?}"
    );

    // A smaller window later moves it in again.
    *harness.world_mut().resource_mut::<WindowSize>() = WindowSize {
        width: 640,
        height: 400,
    };
    harness.step(4);
    let rect = harness.tree().rect(hud).unwrap();
    let viewport = harness.tree().layer_viewport(UiLayer::Debug).unwrap();
    assert!(
        rect.max.x <= viewport.x + 1e-3 && rect.max.y <= viewport.y + 1e-3,
        "{rect:?} in {viewport:?}"
    );
}

#[test]
fn the_views_add_no_text_section() {
    let mut app = app();
    app.add_system(|_| {});
    let mut harness = UiHarness::new(app);
    open_all(&mut harness);
    harness.step(3);
    let draw = harness.harness().draw().expect("a completed frame");
    assert!(draw.text.is_empty(), "{:?}", draw.text);
    assert!(
        draw.ui.commands.len() > 10,
        "the windows paint instead: {}",
        draw.ui.commands.len()
    );
}

/// A 32×32 sprite whose top-left is at `at` in the world, drawn there at
/// the default camera.
fn spawn_sprite(world: &mut World, at: Vec2) -> Entity {
    let asset = world.resource_mut::<AssetRegistry>().register_sprite(
        format!("sprite_{}_{}", at.x, at.y),
        FilterMode::Nearest,
        32,
        32,
        PathBuf::from("test/sprite.png"),
        TextureHandle(0),
        UvRect::FULL,
        None,
        None,
        None,
    );
    world.spawn_with((
        Transform::from_position(at),
        Sprite::new(asset),
        Visibility::default(),
    ))
}

fn selected(harness: &UiHarness) -> Option<Entity> {
    harness.world().resource::<InspectorState>().selected
}

/// A middle press at `press`, then a move to `then`, in one frame.
fn pick(harness: &mut UiHarness, press: Vec2, then: Vec2) {
    harness.pointer_to(press.x, press.y);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .mouse_down(MouseButton::Middle);
    harness.pointer_to(then.x, then.y);
    harness.step(1);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .mouse_up(MouseButton::Middle);
    harness.step(1);
}

#[test]
fn a_press_the_ui_took_never_picks_and_a_world_press_picks_where_it_happened() {
    let mut harness = UiHarness::new(app());
    let (target, other) = {
        let world = harness.world_mut();
        world.resource_mut::<InspectorState>().enabled = true;
        let target = spawn_sprite(world, Vec2::new(600.0, 300.0));
        let other = spawn_sprite(world, Vec2::new(900.0, 300.0));
        (target, other)
    };
    let game = {
        let tree = harness.tree_mut();
        tree.add_root(
            UiLayer::Game,
            LayoutStyle::default()
                .with_size(Sizing::Px(200.0), Sizing::Px(100.0))
                .anchored(Anchor::TopStart, Vec2::new(300.0, 0.0)),
        )
    };
    harness.step(2);
    harness
        .world_mut()
        .resource_mut::<InspectorState>()
        .selected = Some(other);
    let over_target = Vec2::new(616.0, 316.0);

    pick(&mut harness, Vec2::new(350.0, 50.0), over_target);
    assert_eq!(
        selected(&harness),
        Some(other),
        "over a game root, the UI's"
    );
    let window = roots(&harness)[2].unwrap();
    let title: Rect = harness
        .tree()
        .rect(harness.tree().children(window)[0])
        .unwrap();
    let over_window = (title.min + title.max) * 0.5;
    pick(&mut harness, over_window, over_target);
    assert_eq!(
        selected(&harness),
        Some(other),
        "over the inspector's window"
    );
    assert!(harness.tree().contains(game));

    pick(&mut harness, over_target, over_window);
    assert_eq!(
        selected(&harness),
        Some(target),
        "a world press picks where it happened, not where the pointer ended"
    );
}

#[test]
fn without_the_ui_plugin_the_raw_press_picks() {
    let app = App::with_plugins(
        Config::default(),
        DefaultPlugins::set().without::<UiPlugin>(),
    )
    .unwrap();
    let mut harness = UiHarness::new(app);
    let target = {
        let world = harness.world_mut();
        world.resource_mut::<InspectorState>().enabled = true;
        spawn_sprite(world, Vec2::new(600.0, 300.0))
    };
    harness.step(2);
    pick(&mut harness, Vec2::new(616.0, 316.0), Vec2::new(10.0, 10.0));
    assert_eq!(selected(&harness), Some(target));
}

#[test]
fn the_graph_takes_six_vertices_per_interval_up_to_240() {
    let mut out = Vec::new();
    graph_vertices(&[10.0, 20.0], 80.0, &mut out);
    assert_eq!(out.len(), 12 + 12, "two bars, two guide lines");
    // Two bars at the right edge, on the budget ramp: the second over
    // 16.7 ms is yellow.
    assert!((out[0].position.x - 238.0 / 240.0).abs() < 1e-6);
    assert!((out[11].position.x - 239.0 / 240.0).abs() < 1e-6);
    assert_eq!(out[0].color, GREEN);
    assert_eq!(out[6].color, YELLOW);
    // Against 33.3 ms: a 10 ms bar rises 10 / 33.3 of the height.
    assert!((out[0].position.y - (1.0 - 10.0 / 33.3)).abs() < 1e-6);
    assert_eq!(out[2].position.y, 1.0, "from the bottom");

    let samples: Vec<f32> = (0..300).map(|i| i as f32).collect();
    graph_vertices(&samples, 80.0, &mut out);
    assert_eq!(out.len(), GRAPH_SAMPLES * 6 + 12);
    assert_eq!(out[0].position.x, 0.0, "the oldest shown at the left");
    assert_eq!(out[1].position.y, 1.0 - 60.0 / 299.0, "against the tallest");
}

// ---- step 5: the log console and the UI inspector ----

#[test]
fn f7_and_f8_toggle_the_ui_inspector_and_the_log_console() {
    let mut harness = UiHarness::new(app());
    tap(&mut harness, KeyCode::F7);
    tap(&mut harness, KeyCode::F8);
    let inspector = harness.world().resource::<UiInspector>().window().unwrap();
    let console = harness.world().resource::<LogConsole>().window().unwrap();
    assert_eq!(harness.tree().accessible_name(inspector), Some("UI"));
    assert_eq!(harness.tree().accessible_name(console), Some("Log"));
    tap(&mut harness, KeyCode::F7);
    tap(&mut harness, KeyCode::F8);
    assert_eq!(harness.world().resource::<UiInspector>().window(), None);
    assert_eq!(harness.world().resource::<LogConsole>().window(), None);
    assert!(!harness.tree().contains(inspector));
}

/// A world the views' frame lays out in, with a console on `feed`.
fn console_world(feed: crate::logging::ConsoleFeed) -> World {
    let mut world = World::new();
    world.insert_resource(UiTree::new());
    world.insert_resource(WindowSize {
        width: 1280,
        height: 720,
    });
    world.insert_resource(LogConsole::with_feed(Some(feed)));
    world
}

#[test]
fn the_console_drains_while_hidden_keeps_its_capacity_and_colours_lines() {
    let (sink, feed) = crate::logging::console_channel(16);
    let mut world = console_world(feed);
    world.resource_mut::<LogConsole>().capacity = 3;
    for record in [
        "[2026-10-15T12:00:01Z INFO  a] one\n",
        "[2026-10-15T12:00:02Z WARN  a] two\n",
        "[2026-10-15T12:00:03Z ERROR a] three\n",
        "[2026-10-15T12:00:04Z INFO  a] four\n",
    ] {
        sink.send(record.as_bytes());
    }
    update(&mut world);
    let console = world.resource::<LogConsole>();
    assert_eq!(console.window(), None, "hidden");
    assert_eq!(
        console.lines().collect::<Vec<_>>(),
        [
            "[2026-10-15T12:00:02Z WARN  a] two",
            "[2026-10-15T12:00:03Z ERROR a] three",
            "[2026-10-15T12:00:04Z INFO  a] four"
        ],
        "drained raw while hidden, the oldest out"
    );

    world.resource_mut::<LogConsole>().enabled = true;
    update(&mut world);
    let window = world.resource::<LogConsole>().window.as_ref().unwrap();
    let tree = world.resource::<UiTree>();
    let colors: Vec<Option<[u8; 4]>> = tree
        .children(window.columns[3])
        .iter()
        .map(|label| tree.overrides(*label).unwrap().text.color)
        .collect();
    assert_eq!(colors, [Some(WARN_TEXT), Some(ERROR_TEXT), Some(VALUE)]);
}

#[test]
fn the_console_header_counts_lines_and_drops_or_names_no_logger() {
    let (sink, feed) = crate::logging::console_channel(2);
    let mut world = console_world(feed);
    world.resource_mut::<LogConsole>().enabled = true;
    for n in 0..3 {
        sink.send(format!("[t INFO  a] {n}\n").as_bytes());
    }
    update(&mut world);
    let header = world
        .resource::<LogConsole>()
        .window
        .as_ref()
        .unwrap()
        .header
        .unwrap();
    let text = |world: &World| -> String {
        let tree = world.resource::<UiTree>();
        tree.text(header)
            .map(|text| text.spans.iter().map(|span| span.text.as_str()).collect())
            .unwrap_or_default()
    };
    assert_eq!(text(&world), "2 lines · 1 dropped");

    world.insert_resource(LogConsole {
        enabled: true,
        ..LogConsole::default()
    });
    update(&mut world);
    let header = world
        .resource::<LogConsole>()
        .window
        .as_ref()
        .unwrap()
        .header
        .unwrap();
    let tree = world.resource::<UiTree>();
    assert_eq!(tree.text(header).unwrap().spans[0].text, "no engine logger");
}

#[test]
fn the_console_follows_its_end_until_the_wheel_moves_it_off() {
    let (sink, feed) = crate::logging::console_channel(256);
    let mut harness = UiHarness::new(app());
    harness.world_mut().insert_resource(LogConsole {
        enabled: true,
        refresh_interval_ms: 0.0,
        ..LogConsole::with_feed(Some(feed))
    });
    let mut next = 0;
    let mut log = |harness: &mut UiHarness, lines: u32| {
        for _ in 0..lines {
            sink.send(format!("[t INFO  a] line {next}\n").as_bytes());
            next += 1;
        }
        harness.step(2);
    };
    log(&mut harness, 40);
    let window = harness
        .world()
        .resource::<LogConsole>()
        .window
        .as_ref()
        .unwrap()
        .body;
    let scroll = |harness: &UiHarness| {
        let tree = harness.tree();
        (
            tree.scroll(window).unwrap(),
            tree.scroll_max(window).unwrap(),
        )
    };
    let (offset, max) = scroll(&harness);
    assert!(max > 0.0, "40 lines overflow 45% of 720");
    assert_eq!(offset, max, "at the end");
    log(&mut harness, 10);
    let (offset, grown) = scroll(&harness);
    assert!(grown > max);
    assert_eq!(offset, grown, "following it");

    let body = harness.tree().visual_rect(window).unwrap();
    let centre = (body.min + body.max) * 0.5;
    harness.pointer_to(centre.x, centre.y);
    harness.wheel(1.0);
    harness.step(1);
    let (moved, _) = scroll(&harness);
    assert_eq!(moved, grown - 40.0, "the wheel moved it off the end");
    log(&mut harness, 10);
    assert_eq!(scroll(&harness).0, moved, "where the wheel left it");

    harness.wheel(-100.0);
    harness.step(1);
    log(&mut harness, 10);
    let (offset, max) = scroll(&harness);
    assert_eq!(offset, max, "the end again, and following");
}

/// A game root with a 120×40 panel at (300, 100); returns the panel.
fn game_panel(harness: &mut UiHarness) -> WidgetId {
    let tree = harness.tree_mut();
    let root = tree.add_root(
        UiLayer::Game,
        LayoutStyle::default()
            .with_size(Sizing::Px(600.0), Sizing::Px(400.0))
            .anchored(Anchor::TopStart, Vec2::ZERO),
    );
    tree.panel(
        root,
        LayoutStyle::default()
            .with_size(Sizing::Px(120.0), Sizing::Px(40.0))
            .anchored(Anchor::TopStart, Vec2::new(300.0, 100.0)),
    )
}

/// A middle click at `at`, in window pixels.
fn middle_click(harness: &mut UiHarness, at: Vec2) {
    harness.pointer_to(at.x, at.y);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .mouse_down(MouseButton::Middle);
    harness.step(1);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .mouse_up(MouseButton::Middle);
    harness.step(1);
}

fn ui_inspector_rows(harness: &UiHarness) -> (String, Vec<(String, String)>) {
    let inspector = harness.world().resource::<UiInspector>();
    let window = inspector.window.as_ref().unwrap();
    let tree = harness.tree();
    let text = |id: WidgetId| -> String {
        tree.text(id)
            .map(|text| text.spans.iter().map(|span| span.text.as_str()).collect())
            .unwrap_or_default()
    };
    let names = tree.children(window.columns[0]);
    let values = tree.children(window.columns[1]);
    let rows = names
        .iter()
        .zip(values)
        .map(|(name, value)| (text(*name), text(*value)))
        .collect();
    (text(window.header.unwrap()), rows)
}

#[test]
fn a_middle_click_over_a_widget_picks_it_and_over_nothing_does_not() {
    let mut harness = UiHarness::new(app());
    let panel = game_panel(&mut harness);
    harness.world_mut().resource_mut::<UiInspector>().enabled = true;
    harness.step(2);
    assert_eq!(
        ui_inspector_rows(&harness).0,
        "middle-click a widget to pick"
    );
    middle_click(&mut harness, Vec2::new(350.0, 120.0));
    assert_eq!(
        harness.world().resource::<UiInspector>().picked,
        Some(panel)
    );
    middle_click(&mut harness, Vec2::new(900.0, 650.0));
    assert_eq!(
        harness.world().resource::<UiInspector>().picked,
        Some(panel),
        "nothing there: no new pick"
    );
    // Its own window is a widget like any other.
    let own = harness.world().resource::<UiInspector>().window().unwrap();
    let title = harness.tree().children(own)[0];
    let rect = harness.tree().rect(title).unwrap();
    middle_click(&mut harness, (rect.min + rect.max) * 0.5);
    let picked = harness.world().resource::<UiInspector>().picked.unwrap();
    assert_eq!(harness.tree().root_of(picked), Some(own));
}

#[test]
fn the_panel_rows_show_the_rects_look_pending_work_and_batch() {
    let mut harness = UiHarness::new(app());
    let panel = game_panel(&mut harness);
    let label = harness.tree_mut().label(panel, "hi");
    harness.world_mut().resource_mut::<UiInspector>().enabled = true;
    harness.step(3);
    let at = harness.tree().rect(label).unwrap();
    middle_click(&mut harness, (at.min + at.max) * 0.5);
    assert_eq!(
        harness.world().resource::<UiInspector>().picked,
        Some(label)
    );
    let (header, rows) = ui_inspector_rows(&harness);
    assert_eq!(
        header,
        format!("widget {}:{}", label.index(), label.generation())
    );
    let value = |name: &str| -> String {
        rows.iter()
            .find(|(row, _)| row == name)
            .unwrap_or_else(|| panic!("no {name} row in {rows:?}"))
            .1
            .clone()
    };
    assert!(
        value("widget").contains("Label Label Game root"),
        "{}",
        value("widget")
    );
    assert_eq!(
        value("layout"),
        format!(
            "({:.1}, {:.1}) {:.1}×{:.1}",
            at.min.x,
            at.min.y,
            at.width(),
            at.height()
        )
    );
    assert_eq!(value("visual"), value("layout"));
    assert_eq!(value("look"), "bg #00000000 border #00000000 0.0 r 0.0");
    assert_eq!(value("text"), "sans 16.0/20.0 #ebebf0ff");
    assert_eq!(value("flags"), "hit");
    assert_eq!(value("pending"), "none");
    assert_eq!(
        value("batch"),
        "batch 0",
        "the game's run, before the windows'"
    );

    let (_, rows) = {
        let world = harness.world_mut();
        world.resource_mut::<UiInspector>().picked = Some(panel);
        harness.step(1);
        ui_inspector_rows(&harness)
    };
    let panel_value = |name: &str| {
        rows.iter()
            .find(|(row, _)| row == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(panel_value("text"), None, "a panel has no text row");
    assert_eq!(panel_value("batch").as_deref(), Some("none"));
}

#[test]
fn a_pick_made_before_its_first_layout_refreshes_once_it_is_laid_out() {
    let mut harness = UiHarness::new(app());
    harness.world_mut().resource_mut::<UiInspector>().enabled = true;
    harness.step(2);
    let panel = game_panel(&mut harness);
    harness.world_mut().resource_mut::<UiInspector>().picked = Some(panel);
    harness.step(3);
    let at = harness.tree().rect(panel).unwrap();
    let (_, rows) = ui_inspector_rows(&harness);
    let layout = rows
        .iter()
        .find(|(name, _)| name == "layout")
        .map(|(_, value)| value.clone());
    assert_eq!(
        layout,
        Some(format!(
            "({:.1}, {:.1}) {:.1}×{:.1}",
            at.min.x,
            at.min.y,
            at.width(),
            at.height()
        )),
        "the rows refresh once the pick is laid out, not 500 ms later"
    );
}

#[test]
fn a_removed_widget_clears_the_pick() {
    let mut harness = UiHarness::new(app());
    let panel = game_panel(&mut harness);
    harness.world_mut().resource_mut::<UiInspector>().enabled = true;
    harness.step(2);
    middle_click(&mut harness, Vec2::new(350.0, 120.0));
    assert_eq!(
        harness.world().resource::<UiInspector>().picked,
        Some(panel)
    );
    harness.tree_mut().remove(panel);
    harness.step(1);
    assert_eq!(harness.world().resource::<UiInspector>().picked, None);
    assert_eq!(
        ui_inspector_rows(&harness).0,
        "middle-click a widget to pick"
    );
}

// ---- M58 step 2: the look ----

/// An sRGB channel in linear light, as the overlay's shader decodes it
/// (`tungsten-render/src/ui.wgsl`'s `srgb_to_linear`).
fn linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `top` alpha-blended over `bottom` in linear light, as the overlay
/// blends on its sRGB target.
fn over(top: [u8; 4], bottom: [f64; 3]) -> [f64; 3] {
    let alpha = f64::from(top[3]) / 255.0;
    std::array::from_fn(|i| alpha * linear(top[i]) + (1.0 - alpha) * bottom[i])
}

/// WCAG 2 relative luminance of a linear colour.
fn luminance(rgb: [f64; 3]) -> f64 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// The contrast of `text` on `background`, both linear.
fn contrast(text: [u8; 4], background: [f64; 3]) -> f64 {
    let (a, b) = (luminance(over(text, background)), luminance(background));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The lowest contrast of every palette text colour on the panel at
/// `panel_alpha`, the title bar and a striped row, and of each level's hue
/// on its badge over a striped row, each over a black and over a white
/// scene: the eight backgrounds; with where it was read.
fn worst_contrast(panel_alpha: u8) -> (f64, String) {
    let badges = [RED, AMBER, CYAN, VIOLET, LABEL];
    let mut worst = (f64::INFINITY, String::new());
    for (scene, backdrop) in [("black", [0.0; 3]), ("white", [1.0; 3])] {
        let [r, g, b, _] = PANEL;
        let panel = over([r, g, b, panel_alpha], backdrop);
        let stripe = over(STRIPE_VEIL, panel);
        let shared = [
            ("panel", panel),
            ("title bar", over(TITLE_VEIL, panel)),
            ("stripe", stripe),
        ];
        for text in TEXT_COLORS {
            let [r, g, b, _] = text;
            let badge = badges
                .contains(&text)
                .then(|| ("badge", over([r, g, b, BADGE_ALPHA], stripe)));
            for (name, background) in shared.into_iter().chain(badge) {
                let ratio = contrast(text, background);
                if ratio < worst.0 {
                    worst = (ratio, format!("{text:?} on the {name} over {scene}"));
                }
            }
        }
    }
    worst
}

#[test]
fn every_palette_text_colour_keeps_4_5_to_1_composited_in_linear_light() {
    let (worst, at) = worst_contrast(PANEL[3]);
    assert!(worst >= 4.5, "{worst:.2}:1, {at}");
    assert!(
        (worst - 4.99).abs() < 0.01 && at.contains(&format!("{RED:?} on the title bar over white")),
        "the worst case: {worst:.3}:1, {at}"
    );
    // M57's panel alpha fails the same model: in linear light a faint
    // veil lifts near-black a long way.
    let (m57, at) = worst_contrast(245);
    assert!(m57 < 4.5, "{m57:.2}:1, {at}");
    assert!((m57 - 3.81).abs() < 0.01, "{m57:.3}:1, {at}");
}

fn text_of(tree: &UiTree, id: WidgetId) -> String {
    tree.text(id)
        .map(|text| text.spans.iter().map(|span| span.text.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn the_title_bar_holds_the_title_the_key_and_two_buttons_neither_focusable() {
    let mut harness = UiHarness::new(app());
    harness.world_mut().resource_mut::<DebugHud>().enabled = true;
    harness.step(2);
    let hud = roots(&harness)[0].unwrap();
    let tree = harness.tree();
    let title = tree.children(hud)[0];
    let parts = tree.children(title).to_vec();
    assert_eq!(parts.len(), 5, "title, spacer, key, collapse, close");
    assert_eq!(text_of(tree, parts[0]), "HUD");
    assert_eq!(tree.kind(parts[1]), Some(WidgetKind::Panel));
    assert_eq!(text_of(tree, parts[2]), "F4");
    assert_eq!(tree.resolved_style(parts[2]).unwrap().text.color, LABEL);
    let (key, close) = (tree.rect(parts[2]).unwrap(), tree.rect(parts[4]).unwrap());
    let edge = tree.rect(hud).unwrap().max.x;
    assert!(
        (close.max.x - (edge - 6.0)).abs() < 1e-3 && key.max.x < close.min.x,
        "the key and the buttons at the right: {key:?} {close:?} {edge}"
    );
    for (button, glyph) in [(parts[3], "\u{2013}"), (parts[4], "\u{d7}")] {
        assert_eq!(tree.kind(button), Some(WidgetKind::Button));
        assert!(tree.interactive(button));
        assert!(!tree.focusable(button), "the game keeps the keyboard");
        assert_eq!(text_of(tree, button), glyph);
    }

    // Unbound, the key shows nothing.
    harness
        .world_mut()
        .resource_mut::<tungsten_core::ActionMap>()
        .replace_bindings("engine_toggle_hud", Vec::new());
    harness.step(1);
    assert_eq!(
        harness.tree().visibility(parts[2]),
        Some(tungsten_core::ui::Visibility::Collapsed)
    );
}

#[test]
fn close_turns_a_view_off_and_collapse_collapses_its_header_and_body() {
    use tungsten_core::ui::Visibility;
    let mut harness = UiHarness::new(app());
    harness.world_mut().resource_mut::<DebugHud>().enabled = true;
    harness.step(2);
    let (root, title, collapse, close, fixed, body) = {
        let window = harness
            .world()
            .resource::<DebugHud>()
            .window
            .as_ref()
            .unwrap();
        (
            window.root,
            window.title,
            window.collapse,
            window.close,
            window.fixed.unwrap(),
            window.body,
        )
    };
    harness.click(collapse);
    harness.step(1);
    let tree = harness.tree();
    assert_eq!(tree.visibility(body), Some(Visibility::Collapsed));
    assert_eq!(tree.visibility(fixed), Some(Visibility::Collapsed));
    assert_eq!(text_of(tree, collapse), "+");
    assert_eq!(
        tree.rect(root).unwrap().height(),
        tree.rect(title).unwrap().height(),
        "the title bar alone"
    );
    harness.click(collapse);
    harness.step(1);
    assert_eq!(harness.tree().visibility(body), Some(Visibility::Visible));
    assert_eq!(text_of(harness.tree(), collapse), "\u{2013}");

    harness.click(close);
    harness.step(1);
    let hud = harness.world().resource::<DebugHud>();
    assert!(!hud.enabled, "off, as its key turns it");
    assert_eq!(hud.window(), None);
    assert!(!harness.tree().contains(root));
}

#[test]
fn the_border_brightens_under_the_pointer_and_through_a_drag() {
    let mut harness = UiHarness::new(app());
    harness.world_mut().resource_mut::<DebugHud>().enabled = true;
    harness.step(2);
    let hud = roots(&harness)[0].unwrap();
    let border = |harness: &UiHarness| harness.tree().resolved_style(hud).unwrap().border;
    assert_eq!(border(&harness), BORDER);
    let rect = harness.tree().rect(hud).unwrap();
    harness.pointer_to(rect.max.x - 4.0, rect.max.y - 4.0);
    harness.step(1);
    assert_eq!(border(&harness), BORDER_HOT, "under the pointer");
    harness.pointer_to(10.0, 700.0);
    harness.step(1);
    assert_eq!(border(&harness), BORDER);

    let title = harness.tree().children(hud)[0];
    let at = harness.tree().rect(title).unwrap();
    let from = (at.min + at.max) * 0.5;
    harness.pointer_to(from.x, from.y);
    harness.pointer_down();
    harness.step(1);
    assert_eq!(border(&harness), BORDER_HOT, "pressed");
    for step in 1..=3 {
        harness.pointer_to(from.x - 100.0 * step as f32, from.y + 60.0 * step as f32);
        harness.step(1);
        assert_eq!(border(&harness), BORDER_HOT, "dragging, step {step}");
    }
    harness.pointer_up();
    harness.pointer_to(10.0, 700.0);
    harness.step(2);
    assert_eq!(border(&harness), BORDER, "released and away");
}

#[test]
fn every_window_casts_a_shadow_before_its_box() {
    let mut harness = UiHarness::new(app());
    open_all(&mut harness);
    {
        let world = harness.world_mut();
        world.resource_mut::<LogConsole>().enabled = true;
        world.resource_mut::<UiInspector>().enabled = true;
    }
    harness.step(3);
    let mut windows: Vec<WidgetId> = roots(&harness).into_iter().map(Option::unwrap).collect();
    windows.push(harness.world().resource::<LogConsole>().window().unwrap());
    windows.push(harness.world().resource::<UiInspector>().window().unwrap());
    let list = harness.paint();
    for root in windows {
        assert_eq!(
            harness.tree().shadow(root),
            Some(UiShadow::new(SHADOW, 10.0, Vec2::new(0.0, 3.0)))
        );
        let rect = harness.tree().rect(root).unwrap();
        let at = list
            .commands
            .iter()
            .position(|command| matches!(command, PaintCommand::Rect { rect: r, .. } if *r == rect))
            .expect("the root's box");
        assert!(
            matches!(list.commands[at - 1], PaintCommand::Mesh { count: 54, .. }),
            "the shadow just before it"
        );
    }
}

#[test]
fn data_rows_alternate_a_stripe_and_a_header_row_starts_them_again() {
    let mut harness = UiHarness::new(app());
    harness.world_mut().resource_mut::<UiInspector>().enabled = true;
    harness.step(2);
    let rows = vec![
        Row::data(vec![Cell::new("a"), Cell::new("1")]),
        Row::data(vec![Cell::new("b"), Cell::new("2")]),
        Row::data(vec![Cell::new("c"), Cell::new("3")]),
        Row {
            cells: vec![Cell::new("group")],
            header: true,
            indent: false,
        },
        Row {
            cells: vec![Cell::new("d"), Cell::new("4")],
            header: false,
            indent: true,
        },
        Row {
            cells: vec![Cell::new("e"), Cell::new("5")],
            header: false,
            indent: true,
        },
    ];
    let world = harness.world_mut();
    let mut tree = world.remove_resource::<UiTree>().unwrap();
    {
        let window = world.resource::<UiInspector>().window.as_ref().unwrap();
        write_table(&mut tree, window, &rows);
    }
    world.insert_resource(tree);
    harness.layout();
    let window = harness
        .world()
        .resource::<UiInspector>()
        .window
        .as_ref()
        .unwrap();
    let tree = harness.tree();
    for column in &window.columns {
        let cells = tree.children(*column);
        let fills: Vec<[u8; 4]> = cells
            .iter()
            .map(|cell| tree.resolved_style(*cell).unwrap().background)
            .collect();
        let (clear, veil) = ([0, 0, 0, 0], STRIPE_VEIL);
        assert_eq!(fills, [clear, veil, clear, clear, clear, veil]);
        let header = tree.resolved_style(cells[3]).unwrap();
        assert_eq!(header.text.weight, 600, "a header row is strong");
        let heights: Vec<f32> = cells
            .iter()
            .map(|cell| tree.rect(*cell).unwrap().height())
            .collect();
        assert!(heights.iter().all(|height| *height == 20.0), "{heights:?}");
    }
    let names = tree.children(window.columns[0]);
    let start = |cell: WidgetId| tree.layout_style(cell).unwrap().padding.start;
    assert_eq!(start(names[0]), 6.0);
    assert_eq!(start(names[4]), 18.0, "indented 12 more");
    let values = tree.children(window.columns[1]);
    assert_eq!(start(values[4]), 6.0, "the first cell alone");
    assert_eq!(text_of(tree, values[3]), "", "a missing cell is empty");
}

// ---- M58 step 3: scale, zoom, places, docking and snap ----

/// A press and release of `code` with Control held, a frame each.
fn ctrl_tap(harness: &mut UiHarness, code: KeyCode) {
    use tungsten_core::input::Modifiers;
    let input = harness.world_mut().resource_mut::<InputState>();
    input.set_modifiers(Modifiers::CONTROL);
    input.key_down(code);
    harness.step(1);
    let input = harness.world_mut().resource_mut::<InputState>();
    input.key_up(code);
    input.set_modifiers(Modifiers::empty());
    harness.step(1);
}

/// A click at the centre of where `id` paints, in window pixels.

/// A harness whose window is `width`×`height` at an OS scale of `os`.
fn sized(app: App, width: u32, height: u32, os: f32) -> UiHarness {
    let mut harness = UiHarness::new(app);
    let world = harness.world_mut();
    *world.resource_mut::<WindowSize>() = WindowSize { width, height };
    world.resource_mut::<ScaleFactor>().set_os(os);
    harness
}

#[test]
fn the_debug_scale_follows_the_os_scale_and_steps_down_on_small_windows() {
    for (width, height, os, step) in [
        (1280, 720, 1.0, 1.0),
        (1280, 720, 1.75, 0.75),
        (1920, 1080, 1.75, 1.0),
        (2560, 1440, 1.75, 1.0),
        (800, 600, 1.0, 0.75),
    ] {
        let mut harness = sized(app(), width, height, os);
        harness.world_mut().resource_mut::<DebugHud>().enabled = true;
        harness.step(2);
        let tree = harness.tree();
        assert_eq!(tree.debug_scale(), step, "{width}×{height} at {os}");
        let expected = Vec2::new(width as f32, height as f32) / (os * step);
        let viewport = tree.layer_viewport(UiLayer::Debug).unwrap();
        assert!((viewport - expected).length() < 0.1, "{viewport:?}");
    }
    let viewport = Vec2::new(1280.0, 720.0) / 1.75 / 0.75;
    assert!((viewport.x - 975.2).abs() < 0.1 && (viewport.y - 548.6).abs() < 0.1);
    assert_eq!(step_for(Vec2::new(960.0, 540.0)), 1.0);
    assert_eq!(step_for(Vec2::new(850.0, 480.0)), 0.875);
    assert_eq!(step_for(Vec2::new(300.0, 200.0)), 0.75, "the floor's step");
}

#[test]
fn a_larger_ui_and_text_scale_leave_the_views_at_their_own_size() {
    let run = |ui: f32| {
        let mut harness = UiHarness::new(app());
        {
            let world = harness.world_mut();
            world.resource_mut::<ScaleFactor>().set_ui_scale(ui);
            world.resource_mut::<DebugHud>().enabled = true;
        }
        harness.tree_mut().set_text_scale(ui);
        harness.step(4);
        let hud = roots(&harness)[0].unwrap();
        let window = harness
            .world()
            .resource::<DebugHud>()
            .window
            .as_ref()
            .unwrap();
        let cell = harness.tree().children(window.columns[1])[0];
        let text = &harness.tree().resolved_style(cell).unwrap().text;
        assert_eq!((text.size, text.line_height), (15.0, 20.0), "ui {ui}");
        let caption = window.caption;
        let scale = harness.world().resource::<ScaleFactor>().get();
        let mut tree = harness.world_mut().remove_resource::<UiTree>().unwrap();
        let mut list = PaintList::default();
        tree.paint(
            scale,
            &tungsten_core::ui::UiDebug::default(),
            &AssetRegistry::new(),
            &mut list,
        );
        let node = tree.text_node(caption).unwrap();
        let run = list
            .commands
            .iter()
            .find_map(|command| match command {
                PaintCommand::Text {
                    node: n,
                    rect,
                    scale: s,
                    ..
                } if *n == node => Some((*rect, *s * list.scale)),
                _ => None,
            })
            .unwrap();
        harness.world_mut().insert_resource(tree);
        (physical(&harness, hud), run)
    };
    let (root, (caption, drawn)) = run(1.0);
    let (larger_root, (larger_caption, larger_drawn)) = run(1.5);
    for (a, b) in [(root, larger_root), (caption, larger_caption)] {
        assert!(
            (a.size() - b.size()).abs().max_element() < 0.01,
            "{a:?} and {b:?}"
        );
    }
    assert!(
        (drawn - larger_drawn).abs() < 1e-4,
        "{drawn} and {larger_drawn}"
    );
}

#[test]
fn the_zoom_keys_step_by_eighths_save_at_once_and_need_ctrl_and_an_open_view() {
    let dir = tempdir();
    let file = dir.join("settings").join(DEBUG_WINDOWS_FILE);
    let mut harness = UiHarness::new(app_at(&dir));
    harness.world_mut().resource_mut::<DebugHud>().enabled = true;
    harness.step(2);
    let mut seen = Vec::new();
    for code in [KeyCode::Equal, KeyCode::Minus, KeyCode::Digit0] {
        ctrl_tap(&mut harness, code);
        assert_eq!(
            harness.tree().debug_scale(),
            WindowPlacements::load(&file).zoom().unwrap(),
            "saved at once"
        );
        seen.push(harness.tree().debug_scale());
    }
    assert_eq!(seen, [1.125, 1.0, 1.0]);
    ctrl_tap(&mut harness, KeyCode::Equal);
    ctrl_tap(&mut harness, KeyCode::Equal);
    assert_eq!(harness.tree().debug_scale(), 1.25, "each press counted");

    tap(&mut harness, KeyCode::Equal);
    assert_eq!(harness.tree().debug_scale(), 1.25, "no Ctrl, no zoom");
    harness.world_mut().resource_mut::<DebugHud>().enabled = false;
    harness.step(1);
    ctrl_tap(&mut harness, KeyCode::Equal);
    assert_eq!(harness.tree().debug_scale(), 1.25, "every view off");
    assert_eq!(WindowPlacements::load(&file).zoom(), Some(1.25));

    // The range ends at a half and at twice.
    harness.world_mut().resource_mut::<DebugHud>().enabled = true;
    harness.step(1);
    for _ in 0..12 {
        ctrl_tap(&mut harness, KeyCode::Minus);
    }
    assert_eq!(harness.tree().debug_scale(), 0.5);
}

#[test]
fn a_dragged_place_saves_in_os_logical_units_and_a_zoom_keeps_its_top_left() {
    let dir = tempdir();
    let file = dir.join("settings").join(DEBUG_WINDOWS_FILE);
    let mut harness = sized(app_at(&dir), 1280, 720, 1.75);
    harness
        .world_mut()
        .resource_mut::<WindowPlacements>()
        .set_zoom(1.25);
    open_narrow_hud(&mut harness);
    harness.step(3);
    assert_eq!(harness.tree().debug_scale(), 0.9375, "1.25 × 3/4");
    let hud = roots(&harness)[0].unwrap();
    drag(&mut harness, hud, Vec2::new(175.0, 87.5));
    let top_left = physical(&harness, hud).min;
    let saved = WindowPlacements::load(&file).get("hud").unwrap();
    assert!(
        (saved * 1.75 - top_left).length() < 0.01,
        "{saved:?} OS-logical units at {top_left:?} pixels"
    );

    ctrl_tap(&mut harness, KeyCode::Equal);
    harness.step(1);
    assert_eq!(harness.tree().debug_scale(), 1.375 * 0.75);
    let after = physical(&harness, hud).min;
    assert!((after - top_left).length() < 0.01, "{after:?} {top_left:?}");
}

/// Whether `a` and `b` share any area.
fn overlap(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

/// The five views' roots, open.
fn five(harness: &UiHarness) -> [WidgetId; 5] {
    let [hud, overlay, inspector] = roots(harness).map(Option::unwrap);
    let world = harness.world();
    [
        hud,
        overlay,
        inspector,
        world.resource::<LogConsole>().window().unwrap(),
        world.resource::<UiInspector>().window().unwrap(),
    ]
}

/// The dock's promise at a debug viewport of 960×540 units or more: no two
/// windows overlap, each corner window within 45% by 60%, the log console
/// a strip the viewport's width less its margins and at most 25% tall, and
/// every window inside the viewport.
fn assert_docked(harness: &UiHarness, when: &str) {
    let tree = harness.tree();
    let viewport = tree.layer_viewport(UiLayer::Debug).unwrap();
    let rects = five(harness).map(|root| tree.rect(root).unwrap());
    for (i, a) in rects.iter().enumerate() {
        assert!(
            a.min.x >= -1e-3
                && a.min.y >= -1e-3
                && a.max.x <= viewport.x + 1e-3
                && a.max.y <= viewport.y + 1e-3,
            "{when}: window {i} {a:?} leaves {viewport:?}"
        );
        for (j, b) in rects.iter().enumerate().skip(i + 1) {
            assert!(
                !overlap(*a, *b),
                "{when}: windows {i} and {j} overlap: {a:?} {b:?}"
            );
        }
    }
    for (i, rect) in rects.iter().enumerate() {
        if i == 3 {
            assert!(
                (rect.width() - (viewport.x - 16.0)).abs() < 1e-3,
                "{when}: {rect:?}"
            );
            assert!(
                rect.height() <= viewport.y * 0.25 + 1e-3,
                "{when}: {rect:?}"
            );
        } else {
            assert!(
                rect.width() <= viewport.x * 0.45 + 1e-3,
                "{when}: {i} {rect:?}"
            );
            assert!(
                rect.height() <= viewport.y * 0.6 + 1e-3,
                "{when}: {i} {rect:?}"
            );
        }
    }
}

#[test]
fn docked_windows_never_overlap_through_a_collapse_an_expansion_and_a_growing_row() {
    for os in [1.0, 1.75] {
        let (sink, feed) = crate::logging::console_channel(256);
        let mut harness = sized(app(), 1280, 720, os);
        open_all(&mut harness);
        {
            let world = harness.world_mut();
            world.insert_resource(LogConsole {
                enabled: true,
                refresh_interval_ms: 0.0,
                ..LogConsole::with_feed(Some(feed))
            });
            world.resource_mut::<UiInspector>().enabled = true;
        }
        for n in 0..60 {
            sink.send(format!("[t INFO  a] line {n}\n").as_bytes());
        }
        for frame in 0..4 {
            harness.step(1);
            assert_docked(&harness, &format!("os {os}, open, frame {frame}"));
        }
        let collapse = harness
            .world()
            .resource::<DebugHud>()
            .window
            .as_ref()
            .unwrap()
            .collapse;
        harness.click(collapse);
        for frame in 0..3 {
            harness.step(1);
            assert_docked(&harness, &format!("os {os}, collapsed, frame {frame}"));
        }
        assert!(
            harness
                .world()
                .resource::<DebugHud>()
                .window
                .as_ref()
                .unwrap()
                .collapsed
        );
        harness.click(collapse);
        for frame in 0..3 {
            harness.step(1);
            assert_docked(&harness, &format!("os {os}, expanded, frame {frame}"));
        }
        harness.world_mut().resource_mut::<DebugHud>().add_row(|_| {
            (0..30)
                .map(|_| crate::debug_hud::HudRow {
                    label: "grow",
                    value: "row".into(),
                })
                .collect()
        });
        harness
            .world_mut()
            .resource_mut::<DebugHud>()
            .refresh_interval_ms = 0.0;
        for frame in 0..4 {
            harness.step(1);
            assert_docked(&harness, &format!("os {os}, grown, frame {frame}"));
        }
    }
}

#[test]
fn a_window_alone_on_its_side_reaches_60_percent() {
    let mut harness = UiHarness::new(app());
    {
        let hud = &mut *harness.world_mut().resource_mut::<DebugHud>();
        hud.enabled = true;
        hud.add_row(|_| {
            (0..40)
                .map(|_| crate::debug_hud::HudRow {
                    label: "row",
                    value: "x".into(),
                })
                .collect()
        });
    }
    harness.step(4);
    let hud = roots(&harness)[0].unwrap();
    assert_eq!(harness.tree().rect(hud).unwrap().height(), 720.0 * 0.6);
}

#[test]
fn a_window_dropped_near_the_margin_snaps_onto_it_and_saves_the_snapped_place() {
    let dir = tempdir();
    let file = dir.join("settings").join(DEBUG_WINDOWS_FILE);
    let mut harness = UiHarness::new(app_at(&dir));
    open_narrow_hud(&mut harness);
    harness.step(2);
    let hud = roots(&harness)[0].unwrap();
    assert_eq!(harness.tree().window_position(hud), Some(Vec2::splat(8.0)));
    drag(&mut harness, hud, Vec2::new(10.0, 100.0));
    assert_eq!(
        harness.tree().window_position(hud),
        Some(Vec2::new(8.0, 108.0)),
        "18 from the edge is 10 from the margin line"
    );
    assert_eq!(
        WindowPlacements::load(&file).get("hud"),
        Some(Vec2::new(8.0, 108.0))
    );
}

#[test]
fn the_five_views_dock_at_their_default_corners() {
    assert_eq!(DebugHud::new().corner, HudCorner::TopLeft);
    assert_eq!(SystemTimingOverlay::default().corner, HudCorner::TopRight);
    assert_eq!(InspectorState::default().corner, HudCorner::BottomLeft);
    assert_eq!(LogConsole::default().corner, HudCorner::BottomLeft);
    assert_eq!(UiInspector::default().corner, HudCorner::BottomRight);
    let mut harness = UiHarness::new(app());
    open_all(&mut harness);
    {
        let world = harness.world_mut();
        world.resource_mut::<LogConsole>().enabled = true;
        world.resource_mut::<UiInspector>().enabled = true;
    }
    harness.step(4);
    let [hud, overlay, inspector, console, ui] =
        five(&harness).map(|root| harness.tree().rect(root).unwrap());
    assert_eq!(hud.min, Vec2::splat(8.0), "top left");
    assert_eq!((overlay.max.x, overlay.min.y), (1272.0, 8.0), "top right");
    assert_eq!(
        (console.min.x, console.max.x, console.max.y),
        (8.0, 1272.0, 712.0),
        "a strip along the bottom"
    );
    assert_eq!(
        (inspector.min.x, inspector.max.y),
        (8.0, console.min.y - 8.0),
        "bottom left, above the strip"
    );
    assert_eq!(
        (ui.max.x, ui.max.y),
        (1272.0, console.min.y - 8.0),
        "bottom right, above the strip"
    );
}

// ---- M58 step 6: the UI inspector in the new look ----

#[test]
fn the_ui_inspectors_rows_take_the_new_look() {
    let mut harness = UiHarness::new(app());
    let panel = game_panel(&mut harness);
    harness.world_mut().resource_mut::<UiInspector>().enabled = true;
    harness.step(2);
    middle_click(&mut harness, Vec2::new(350.0, 120.0));
    assert_eq!(
        harness.world().resource::<UiInspector>().picked,
        Some(panel)
    );
    let window = harness
        .world()
        .resource::<UiInspector>()
        .window
        .as_ref()
        .unwrap();
    let tree = harness.tree();
    let style = |id: WidgetId| tree.resolved_style(id).unwrap().text.clone();
    let header = style(window.header.unwrap());
    assert_eq!((header.weight, header.color), (600, VALUE));
    let names = tree.children(window.columns[0]);
    let values = tree.children(window.columns[1]);
    assert!(names.len() > 5 && names.len() == values.len());
    for (name, value) in names.iter().zip(values) {
        let (name, value) = (style(*name), style(*value));
        assert_eq!((name.color, name.weight, name.size), (LABEL, 400, 15.0));
        assert_eq!((value.color, value.weight, value.size), (VALUE, 400, 15.0));
        assert_eq!(value.line_height, 20.0);
    }
}

// ---- M58 step 7: the log console ----

/// Records in the logger's own format at `level`, from `target`.
fn record(level: &str, target: &str, message: &str) -> String {
    format!("[2026-10-15T12:34:56Z {level:<5} {target}] {message}\n")
}

/// The console window's cell texts by column.
fn console_columns(world: &World) -> Vec<Vec<String>> {
    let window = world.resource::<LogConsole>().window.as_ref().unwrap();
    let tree = world.resource::<UiTree>();
    window
        .columns
        .iter()
        .map(|column| {
            tree.children(*column)
                .iter()
                .map(|id| text_of(tree, *id))
                .collect()
        })
        .collect()
}

/// The views' frame and a layout of `world` through `store`.
fn console_frame(world: &mut World, store: &mut tungsten_core::ui::FixedAdvanceMeasure) {
    update(world);
    world
        .resource_mut::<UiTree>()
        .layout(Vec2::new(1280.0, 720.0), store);
}

#[test]
fn the_console_shows_time_level_short_target_and_message_and_caps_the_target() {
    let (sink, feed) = crate::logging::console_channel(16);
    let mut world = console_world(feed);
    world.resource_mut::<LogConsole>().enabled = true;
    let long = format!("game::{}", "t".repeat(300));
    for line in [
        record("WARN", "game::player", "low health"),
        record("INFO", &long, "two\nlines"),
        "not a record\n".to_string(),
    ] {
        sink.send(line.as_bytes());
    }
    let mut store = tungsten_core::ui::FixedAdvanceMeasure::default();
    console_frame(&mut world, &mut store);
    console_frame(&mut world, &mut store);
    let columns = console_columns(&world);
    assert_eq!(columns[0], ["12:34:56", "12:34:56", ""]);
    assert_eq!(columns[1], ["WARN", "INFO", ""]);
    assert_eq!(columns[2][0], "player", "the target's last segment");
    assert_eq!(columns[3], ["low health", "two \u{2026}", "not a record"]);
    // §2.7's probe, the console's half: the target column stops at 20% of
    // the strip, 1280 less two 8-unit margins.
    let window = world.resource::<LogConsole>().window.as_ref().unwrap();
    let tree = world.resource::<UiTree>();
    let target = tree.rect(window.columns[2]).unwrap();
    assert!((target.width() - 0.2 * 1264.0).abs() < 1e-3, "{target:?}");
    let root = tree.rect(window.root).unwrap();
    assert!((root.width() - 1264.0).abs() < 1e-3, "{root:?}");
    for column in &window.columns {
        for cell in tree.children(*column) {
            assert_eq!(tree.rect(*cell).unwrap().height(), 20.0, "one line");
        }
    }
}

#[test]
fn each_level_wears_its_badge_and_tints_its_message() {
    let (sink, feed) = crate::logging::console_channel(16);
    let mut world = console_world(feed);
    world.resource_mut::<LogConsole>().enabled = true;
    let levels = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
    for level in levels {
        sink.send(record(level, "a", "m").as_bytes());
    }
    update(&mut world);
    let window = world.resource::<LogConsole>().window.as_ref().unwrap();
    let tree = world.resource::<UiTree>();
    let badges = tree.children(window.columns[1]);
    let messages = tree.children(window.columns[3]);
    let hues = [RED, AMBER, CYAN, VIOLET, LABEL];
    let tints = [ERROR_TEXT, WARN_TEXT, VALUE, VALUE, VALUE];
    for (i, level) in levels.into_iter().enumerate() {
        let badge = tree.overrides(badges[i]).unwrap();
        assert_eq!(text_of(tree, badges[i]), level);
        assert_eq!(badge.text.color, Some(hues[i]), "{level}");
        assert_eq!(badge.text.weight, Some(600));
        let [r, g, b, _] = hues[i];
        assert_eq!(badge.background, Some([r, g, b, BADGE_ALPHA]));
        assert_eq!(
            tree.overrides(messages[i]).unwrap().text.color,
            Some(tints[i]),
            "{level}"
        );
    }
}

/// A console harness: the app with a console on a fresh channel, shown and
/// refreshing every frame.
fn console_harness() -> (UiHarness, crate::logging::ConsoleSink) {
    let (sink, feed) = crate::logging::console_channel(1024);
    let mut harness = UiHarness::new(app());
    harness.world_mut().insert_resource(LogConsole {
        enabled: true,
        refresh_interval_ms: 0.0,
        ..LogConsole::with_feed(Some(feed))
    });
    harness.step(2);
    (harness, sink)
}

fn console_controls(harness: &UiHarness) -> Vec<WidgetId> {
    harness
        .world()
        .resource::<LogConsole>()
        .window
        .as_ref()
        .unwrap()
        .controls
        .clone()
}

/// Clicks a console control and runs the frame that takes it.
fn press_control(harness: &mut UiHarness, i: usize) {
    let control = console_controls(harness)[i];
    harness.click(control);
    harness.step(1);
}

#[test]
fn the_harness_clicks_a_debug_control_where_it_paints_under_a_debug_scale() {
    let (sink, feed) = crate::logging::console_channel(1024);
    let mut harness = sized(app(), 1280, 720, 1.75);
    harness.world_mut().insert_resource(LogConsole {
        enabled: true,
        refresh_interval_ms: 0.0,
        ..LogConsole::with_feed(Some(feed))
    });
    harness.step(2);
    sink.send(record("INFO", "a", "m").as_bytes());
    harness.step(1);
    assert_eq!(harness.tree().debug_scale(), 0.75);
    press_control(&mut harness, 5);
    assert!(
        harness.world().resource::<LogConsole>().paused.is_some(),
        "the click lands on pause"
    );
}

#[test]
fn the_controls_line_filters_pauses_and_clears() {
    let (mut harness, sink) = console_harness();
    for level in ["ERROR", "WARN", "INFO", "INFO"] {
        sink.send(record(level, "a", "m").as_bytes());
    }
    harness.step(1);
    let levels = |harness: &UiHarness| console_columns(harness.world())[1].clone();
    assert_eq!(levels(&harness), ["ERROR", "WARN", "INFO", "INFO"]);
    let labels = |harness: &UiHarness| -> Vec<String> {
        console_controls(harness)
            .iter()
            .map(|id| text_of(harness.tree(), *id))
            .collect()
    };
    assert_eq!(
        labels(&harness),
        [
            "[x] ERROR",
            "[x] WARN",
            "[x] INFO",
            "[x] DEBUG",
            "[x] TRACE",
            "pause",
            "clear"
        ]
    );
    let status = |harness: &UiHarness| {
        let window = harness
            .world()
            .resource::<LogConsole>()
            .window
            .as_ref()
            .unwrap();
        text_of(harness.tree(), window.header.unwrap())
    };
    assert_eq!(status(&harness), "4 lines \u{b7} 0 dropped");

    // Each filter hides its level and shows it again.
    for (i, level) in ["ERROR", "WARN", "INFO"].into_iter().enumerate() {
        press_control(&mut harness, i);
        assert!(
            !levels(&harness).iter().any(|shown| shown == level),
            "{level} hidden"
        );
        assert_eq!(labels(&harness)[i], format!("[ ] {level}"));
        let label = harness.tree().children(console_controls(&harness)[i])[0];
        assert_eq!(
            harness.tree().resolved_style(label).unwrap().text.color,
            LABEL
        );
        press_control(&mut harness, i);
        assert_eq!(
            levels(&harness),
            ["ERROR", "WARN", "INFO", "INFO"],
            "{level} back"
        );
    }

    // Pause holds the window while the ring fills; resume catches up.
    press_control(&mut harness, 5);
    assert_eq!(labels(&harness)[5], "resume");
    for _ in 0..3 {
        sink.send(record("INFO", "a", "later").as_bytes());
    }
    harness.step(2);
    assert_eq!(levels(&harness).len(), 4, "held");
    assert_eq!(harness.world().resource::<LogConsole>().lines().count(), 7);
    assert_eq!(status(&harness), "paused \u{b7} 3 new");
    press_control(&mut harness, 5);
    assert_eq!(levels(&harness).len(), 7, "caught up");
    assert_eq!(labels(&harness)[5], "pause");

    // Clear empties the ring and the window.
    press_control(&mut harness, 6);
    assert_eq!(harness.world().resource::<LogConsole>().lines().count(), 0);
    assert!(levels(&harness).is_empty());
    harness.world_mut().resource_mut::<LogConsole>().clear();
    harness.step(1);
    assert!(levels(&harness).is_empty(), "the public clear too");
}

#[test]
fn the_controls_line_stays_in_view_and_takes_a_click_while_the_log_follows_its_end() {
    let (mut harness, sink) = console_harness();
    for n in 0..80 {
        sink.send(record("INFO", "a", &format!("line {n}")).as_bytes());
    }
    harness.step(3);
    let window = harness
        .world()
        .resource::<LogConsole>()
        .window
        .as_ref()
        .unwrap();
    let (root, body) = (window.root, window.body);
    let tree = harness.tree();
    let (offset, max) = (tree.scroll(body).unwrap(), tree.scroll_max(body).unwrap());
    assert!(
        max > 0.0 && offset == max,
        "following its end: {offset} of {max}"
    );
    let pause = console_controls(&harness)[5];
    let shown = tree.visual_rect(pause).unwrap();
    let frame = tree.rect(root).unwrap();
    assert!(
        shown.min.y >= frame.min.y && shown.max.y <= frame.max.y,
        "{shown:?} in {frame:?}"
    );
    press_control(&mut harness, 5);
    assert!(harness.world().resource::<LogConsole>().paused.is_some());
}

#[test]
fn debug_views_keep_their_size_under_a_reference_height() {
    // The template's policy: the UI sized from a 720-unit reference height
    // under a 731×411 floor (`D-184`), at 4K where the base is 3.
    let run = |reference: Option<f32>, ui: f32| {
        let mut harness = sized(app(), 3840, 2160, 1.0);
        {
            let world = harness.world_mut();
            let scale = world.resource_mut::<ScaleFactor>();
            scale.set_reference_height(reference);
            scale.set_min_viewport(reference.map(|_| Vec2::new(731.0, 411.0)));
            scale.set_ui_scale(ui);
            world.resource_mut::<DebugHud>().enabled = true;
        }
        harness.step(4);
        let hud = roots(&harness)[0].unwrap();
        (
            harness.world().resource::<ScaleFactor>().get(),
            harness.tree().debug_scale(),
            physical(&harness, hud),
        )
    };
    let (plain_factor, plain_debug, plain) = run(None, 1.0);
    assert_eq!((plain_factor, plain_debug), (1.0, 1.0));
    for ui in [1.0, 3.0] {
        let (factor, debug, root) = run(Some(720.0), ui);
        assert!(factor > 2.9, "the window-relative factor applies: {factor}");
        assert!(
            (factor * debug - 1.0).abs() < 1e-5,
            "the views draw at the OS scale: {factor} × {debug}"
        );
        assert!(
            (root.size() - plain.size()).abs().max_element() < 0.01,
            "ui {ui}: {root:?} and {plain:?}"
        );
    }
}
