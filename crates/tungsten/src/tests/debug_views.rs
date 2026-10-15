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

/// Whether every node under `root` resolved to the view's look, not the
/// theme's: the view's text everywhere, and no hostile fill or border.
fn assert_own_look(tree: &UiTree, root: WidgetId, size: f32, line_height: f32, color: [u8; 4]) {
    for id in subtree(tree, root) {
        let style = tree.resolved_style(id).expect("resolved");
        let what = format!("{id:?} {:?}", tree.kind(id));
        assert_ne!(style.background, [255, 0, 0, 255], "{what}");
        assert_ne!(style.border, [0, 255, 0, 255], "{what}");
        assert_ne!(style.border_width, 7.0, "{what}");
        assert_ne!(style.corner_radius, 9.0, "{what}");
        let text = &style.text;
        assert_eq!(text.family, crate::ENGINE_FONT_ID, "{what}");
        assert_eq!((text.size, text.line_height), (size, line_height), "{what}");
        assert_eq!((text.weight, text.italic), (400, false), "{what}");
        assert_eq!(text.color, color, "{what}");
        assert!(
            matches!(text.layout.align, TextAlign::Start | TextAlign::End),
            "{what}"
        );
        assert_eq!(text.layout.wrap, TextWrap::None, "{what}");
        assert_eq!(text.layout.overflow, TextOverflow::Clip, "{what}");
    }
}

#[test]
fn every_node_a_view_builds_keeps_its_look_under_a_game_theme() {
    let mut harness = UiHarness::new(app());
    harness.tree_mut().set_theme(hostile_theme());
    open_all(&mut harness);
    harness.step(3);
    let [hud, overlay, inspector] = roots(&harness).map(Option::unwrap);
    let tree = harness.tree();
    assert_own_look(tree, hud, 26.0, 30.0, [240, 240, 240, 240]);
    assert_own_look(tree, overlay, 28.0, 32.0, [240, 240, 240, 240]);
    assert_own_look(tree, inspector, 28.0, 32.0, [240, 240, 240, 240]);
    assert_eq!(tree.resolved_style(hud).unwrap().background, BACKGROUND);
    assert_eq!(tree.resolved_style(hud).unwrap().border, [0, 0, 0, 220]);

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
    assert_own_look(harness.tree(), overlay, 28.0, 32.0, [240, 240, 240, 240]);
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
            matches!(command, PaintCommand::Rect { fill: [240, 240, 240, 120], rect, .. } if rect.width() == 4.0)
        })
        .collect();
    assert_eq!(thumbs.len(), 1, "the overlay's: {thumbs:?}");
}

#[test]
fn views_stack_at_one_corner_and_a_saved_place_wins() {
    let mut harness = UiHarness::new(app());
    {
        let world = harness.world_mut();
        world.resource_mut::<SystemTimingOverlay>().corner = HudCorner::TopRight;
        world.resource_mut::<InspectorState>().corner = HudCorner::TopRight;
    }
    open_all(&mut harness);
    harness.step(4);
    let [hud, overlay, inspector] = roots(&harness).map(Option::unwrap);
    let tree = harness.tree();
    let rect = |id| tree.rect(id).unwrap();
    assert_eq!(rect(hud).min.y, 10.0, "the HUD's padding");
    assert_eq!(rect(hud).max.x, 1270.0);
    assert_eq!(rect(overlay).min.y, rect(hud).max.y + 12.0, "below the HUD");
    assert_eq!(rect(overlay).max.x, 1268.0);
    assert_eq!(rect(inspector).min.y, rect(overlay).max.y + 12.0);

    // A saved place wins, and the next view stacks as if it were not there.
    let mut harness = UiHarness::new(app());
    {
        let world = harness.world_mut();
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
        tree.rect(hud).unwrap().max.y + 12.0,
        "directly below the HUD"
    );
}

/// Drags `root` by its title bar by `by`, in window pixels.
fn drag(harness: &mut UiHarness, root: WidgetId, by: Vec2) {
    let title = harness.tree().children(root)[0];
    let rect = harness.tree().rect(title).unwrap();
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
    drag(&mut harness, hud, Vec2::new(-200.0, 100.0));
    let moved = start + Vec2::new(-200.0, 100.0);
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
    harness.step(3);
    let hud = roots(&harness)[0].unwrap();
    let rect = harness.tree().rect(hud).unwrap();
    assert!(
        rect.min.x >= 0.0 && rect.min.y >= 0.0 && rect.max.x <= 800.0 && rect.max.y <= 600.0,
        "{rect:?}"
    );
    assert!(
        rect.max.x == 800.0 || rect.max.y == 600.0,
        "clamped to the edge"
    );

    // A smaller window later moves it in again.
    *harness.world_mut().resource_mut::<WindowSize>() = WindowSize {
        width: 640,
        height: 400,
    };
    harness.step(3);
    let rect = harness.tree().rect(hud).unwrap();
    assert!(rect.max.x <= 640.0 && rect.max.y <= 400.0, "{rect:?}");
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
    graph_vertices(&[10.0, 20.0], [1, 2, 3, 4], &mut out);
    assert_eq!(out.len(), 12);
    // Two bars at the right edge; the second over 16.7 ms is amber.
    assert!((out[0].position.x - 238.0 / 240.0).abs() < 1e-6);
    assert!((out[11].position.x - 239.0 / 240.0).abs() < 1e-6);
    assert_eq!(out[0].color, [1, 2, 3, 4]);
    assert_eq!(out[6].color, GRAPH_SLOW);
    // Against 33.3 ms: a 10 ms bar rises 10 / 33.3 of the height.
    assert!((out[0].position.y - (1.0 - 10.0 / 33.3)).abs() < 1e-6);
    assert_eq!(out[2].position.y, 1.0, "from the bottom");

    let samples: Vec<f32> = (0..300).map(|i| i as f32).collect();
    graph_vertices(&samples, [1, 2, 3, 4], &mut out);
    assert_eq!(out.len(), GRAPH_SAMPLES * 6);
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

#[test]
fn records_are_coloured_by_their_level_token() {
    assert_eq!(
        level_color("[2026-10-15T00:00:00Z ERROR game] x"),
        Some(ERROR_LINE)
    );
    assert_eq!(
        level_color("[2026-10-15T00:00:00Z WARN  game] x"),
        Some(GRAPH_SLOW)
    );
    assert_eq!(level_color("[2026-10-15T00:00:00Z INFO  game] x"), None);
    assert_eq!(level_color("WARN without the format"), None);
    assert_eq!(level_color("[only]"), None);
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
        "[t INFO  a] one\n",
        "[t WARN  a] two\n",
        "[t ERROR a] three\n",
        "[t INFO  a] four\n",
    ] {
        sink.send(record.as_bytes());
    }
    update(&mut world);
    let console = world.resource::<LogConsole>();
    assert_eq!(console.window(), None, "hidden");
    assert_eq!(
        console.lines().collect::<Vec<_>>(),
        ["[t WARN  a] two", "[t ERROR a] three", "[t INFO  a] four"],
        "drained while hidden, the oldest out"
    );

    world.resource_mut::<LogConsole>().enabled = true;
    update(&mut world);
    let console = world.resource::<LogConsole>();
    let tree = world.resource::<UiTree>();
    let colors: Vec<Option<[u8; 4]>> = console
        .labels
        .iter()
        .map(|label| tree.overrides(*label).unwrap().text.color)
        .collect();
    assert_eq!(
        colors,
        [
            Some(GRAPH_SLOW),
            Some(ERROR_LINE),
            Some([240, 240, 240, 240])
        ]
    );
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

#[test]
fn the_ui_inspector_stacks_under_the_hud() {
    let mut harness = UiHarness::new(app());
    {
        let world = harness.world_mut();
        world.resource_mut::<DebugHud>().enabled = true;
        world.resource_mut::<UiInspector>().enabled = true;
    }
    harness.step(4);
    let hud = harness.world().resource::<DebugHud>().window().unwrap();
    let inspector = harness.world().resource::<UiInspector>().window().unwrap();
    let tree = harness.tree();
    assert_eq!(
        tree.rect(inspector).unwrap().min.y,
        tree.rect(hud).unwrap().max.y + 12.0
    );
    assert_eq!(tree.rect(inspector).unwrap().max.x, 1268.0);
}
