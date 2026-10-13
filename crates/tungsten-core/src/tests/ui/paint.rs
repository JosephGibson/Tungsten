use std::path::PathBuf;

use glam::Vec2;

use super::*;
use crate::assets::{AssetRegistry, SpriteAssetId};
use crate::ui::theme::StyleOverrides;
use crate::ui::{Anchor, FixedAdvanceMeasure, LayoutStyle, Sizing, Visibility};

/// 8 px per glyph at the theme's size 16; lines are 20 tall.
const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);
const WHITE: [u8; 4] = [255, 255, 255, 255];
const SPRITE_UV: UvRect = UvRect {
    min: [0.25, 0.5],
    max: [0.75, 1.0],
};

/// A registry with one 32×32 sprite `name` on page 3 at [`SPRITE_UV`].
fn registry(name: &str, nine_slice: Option<[u32; 4]>) -> (AssetRegistry, SpriteAssetId) {
    let mut registry = AssetRegistry::new();
    let id = registry.register_sprite(
        name.to_owned(),
        FilterMode::Nearest,
        32,
        32,
        PathBuf::from(format!("{name}.png")),
        TextureHandle(3),
        SPRITE_UV,
        None,
        None,
        None,
    );
    registry.set_sprite_nine_slice(name, nine_slice);
    (registry, id)
}

/// Lays the tree out at [`VIEWPORT`] through `store` and paints it.
fn paint_with(
    tree: &mut UiTree,
    store: &mut FixedAdvanceMeasure,
    scale: f32,
    debug: UiDebug,
    sprites: &AssetRegistry,
) -> PaintList {
    tree.layout(VIEWPORT, store);
    let mut list = PaintList::default();
    tree.paint(scale, &debug, sprites, &mut list);
    list
}

/// [`paint_with`] on a fresh store: for a tree laid out once, or laid out
/// again with no change to its text.
fn paint(tree: &mut UiTree, scale: f32, debug: UiDebug, sprites: &AssetRegistry) -> PaintList {
    paint_with(
        tree,
        &mut FixedAdvanceMeasure::default(),
        scale,
        debug,
        sprites,
    )
}

fn fixed(width: f32, height: f32) -> LayoutStyle {
    LayoutStyle::default().with_size(Sizing::Px(width), Sizing::Px(height))
}

fn at(x: f32, y: f32, width: f32, height: f32) -> LayoutStyle {
    fixed(width, height).anchored(Anchor::TopStart, Vec2::new(x, y))
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_pos_size(Vec2::new(x, y), Vec2::new(width, height))
}

fn rect_command(rect: Rect) -> PaintCommand {
    PaintCommand::Rect {
        rect,
        fill: WHITE,
        border: [0, 0, 0, 0],
        border_width: 0.0,
        corner_radius: 0.0,
        clip: 0,
    }
}

fn text_command(rect: Rect) -> PaintCommand {
    PaintCommand::Text {
        node: TextNodeId::new(0, 0),
        rect,
        color: WHITE,
        clip: 0,
    }
}

fn list_of(commands: Vec<PaintCommand>) -> PaintList {
    PaintList {
        scale: 1.0,
        clips: vec![ClipRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        }],
        commands,
    }
}

fn rects(list: &PaintList) -> Vec<Rect> {
    list.commands.iter().map(PaintCommand::rect).collect()
}

fn kinds(list: &PaintList) -> String {
    list.commands
        .iter()
        .map(|command| match command {
            PaintCommand::Rect { .. } => "R",
            PaintCommand::Image { .. } => "I",
            PaintCommand::Text { .. } => "T",
        })
        .collect()
}

#[test]
fn the_rounding_rule_floors_the_min_and_ceils_the_max() {
    assert_eq!(
        to_physical(rect(10.5, 3.0, 33.3, 20.0), 1.5),
        ClipRect {
            x: 15,
            y: 4,
            width: 51,
            height: 31
        }
    );
    assert_eq!(
        to_physical(rect(10.5, 3.0, 33.3, 20.0), 2.0),
        ClipRect {
            x: 21,
            y: 6,
            width: 67,
            height: 40
        }
    );
    assert_eq!(
        to_physical(rect(-1.5, -1.5, 2.0, 2.0), 1.0),
        ClipRect {
            x: -2,
            y: -2,
            width: 3,
            height: 3
        }
    );
    assert_eq!(
        to_physical(rect(0.0, 0.0, 800.0, 600.0), 1.0),
        ClipRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600
        }
    );
}

#[test]
fn clip_intersection_is_empty_when_the_rects_miss() {
    let a = ClipRect {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    let b = ClipRect {
        x: 5,
        y: 5,
        width: 10,
        height: 10,
    };
    assert_eq!(
        a.intersect(b),
        ClipRect {
            x: 5,
            y: 5,
            width: 5,
            height: 5
        }
    );
    let far = ClipRect {
        x: 20,
        y: 20,
        width: 5,
        height: 5,
    };
    assert!(a.intersect(far).is_empty());
    assert!(!a.is_empty());
    assert!(ClipRect::default().is_empty());
}

#[test]
fn a_rear_windows_text_paints_before_a_front_windows_background() {
    let mut tree = UiTree::new();
    let debug = tree.add_root(UiLayer::Debug, LayoutStyle::default());
    tree.label(debug, "fps");
    let rear = tree.add_root(UiLayer::Game, LayoutStyle::default());
    tree.label(rear, "rear");
    let front = tree.add_root(UiLayer::GamePopup, at(50.0, 10.0, 200.0, 100.0));
    tree.label(front, "front");
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());

    assert_eq!(kinds(&list), "RTRTRT");
    assert_eq!(
        rects(&list),
        [
            rect(0.0, 0.0, 800.0, 20.0),
            rect(0.0, 0.0, 800.0, 20.0),
            rect(50.0, 10.0, 200.0, 100.0),
            rect(50.0, 10.0, 200.0, 20.0),
            rect(0.0, 0.0, 800.0, 20.0),
            rect(0.0, 0.0, 800.0, 20.0),
        ]
    );
    assert_eq!(list.text_batches(), [1..2, 3..4, 5..6]);
    assert_eq!(list.scale, 1.0);
    assert_eq!(list.clips.len(), 1, "no node clips its children");
    assert!(!tree.is_paint_dirty(), "paint clears the flag");
}

#[test]
fn hidden_and_collapsed_subtrees_paint_nothing() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let hidden = tree.panel(root, fixed(100.0, 30.0));
    tree.label(hidden, "hidden");
    let collapsed = tree.panel(root, fixed(100.0, 30.0));
    tree.label(collapsed, "collapsed");
    let shown = tree.label(root, "shown");
    tree.set_visibility(hidden, Visibility::Hidden);
    tree.set_visibility(collapsed, Visibility::Collapsed);
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());

    assert_eq!(kinds(&list), "RT");
    assert_eq!(
        rects(&list),
        [rect(0.0, 0.0, 800.0, 50.0), rect(0.0, 30.0, 800.0, 20.0)]
    );
    assert_eq!(tree.rect(hidden), Some(rect(0.0, 0.0, 100.0, 30.0)));
    assert_eq!(tree.rect(collapsed), None);
    assert_eq!(tree.rect(shown), Some(rect(0.0, 30.0, 800.0, 20.0)));
}

#[test]
fn a_label_with_a_visible_background_paints_its_box_before_its_text() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let label = tree.label(root, "boxed");
    tree.set_overrides(
        label,
        StyleOverrides {
            background: Some([10, 20, 30, 255]),
            ..StyleOverrides::default()
        },
    );
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(kinds(&list), "RRT");
    assert!(matches!(
        list.commands[1],
        PaintCommand::Rect {
            fill: [10, 20, 30, 255],
            border_width: 0.0,
            ..
        }
    ));
}

#[test]
fn clipping_nodes_push_the_intersection_and_an_empty_one_skips_the_children() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, at(0.0, 0.0, 100.0, 100.0));
    tree.set_clip_children(root, true);
    let inside = tree.panel(root, at(50.0, 50.0, 100.0, 100.0));
    tree.set_clip_children(inside, true);
    tree.label(inside, "clipped");
    let outside = tree.panel(root, at(200.0, 200.0, 50.0, 50.0));
    tree.set_clip_children(outside, true);
    tree.label(outside, "never");
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());

    assert_eq!(
        list.clips,
        [
            ClipRect {
                x: 0,
                y: 0,
                width: 800,
                height: 600
            },
            ClipRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100
            },
            ClipRect {
                x: 50,
                y: 50,
                width: 50,
                height: 50
            },
        ]
    );
    assert_eq!(kinds(&list), "RRTR");
    let clips: Vec<u32> = list.commands.iter().map(PaintCommand::clip).collect();
    assert_eq!(clips, [0, 1, 2, 1]);
    assert_eq!(
        list.clip_of(&list.commands[2]),
        ClipRect {
            x: 50,
            y: 50,
            width: 50,
            height: 50
        }
    );
    assert_eq!(
        list.commands[3].rect(),
        rect(200.0, 200.0, 50.0, 50.0),
        "the outside panel paints under clip 1; its children do not"
    );
}

#[test]
fn primitive_edges_and_clips_agree_at_every_scale() {
    for scale in [1.0, 1.5, 2.0] {
        let mut tree = UiTree::new();
        let root = tree.add_root(UiLayer::Game, at(10.5, 3.0, 33.3, 20.0));
        tree.set_clip_children(root, true);
        tree.label(root, "x");
        let list = paint(&mut tree, scale, UiDebug::default(), &AssetRegistry::new());
        let logical = tree.rect(root).unwrap();
        let clip = to_physical(logical, scale);
        assert_eq!(list.clips[1], clip, "scale {scale}");
        let PaintCommand::Rect {
            rect,
            border_width,
            corner_radius,
            ..
        } = list.commands[0]
        else {
            panic!("the root paints a rect");
        };
        assert_eq!(rect.min, logical.min * scale);
        assert_eq!(rect.max, logical.max * scale);
        assert!(rect.min.x >= clip.x as f32 && rect.min.y >= clip.y as f32);
        assert!(rect.max.x <= (clip.x + clip.width) as f32);
        assert!(rect.max.y <= (clip.y + clip.height) as f32);
        assert_eq!(border_width, scale, "the theme's 1 px border, scaled");
        assert_eq!(corner_radius, 4.0 * scale);
        assert_eq!(list.scale, scale);
    }
}

#[test]
fn text_over_its_own_panel_joins_the_open_batch() {
    let list = list_of(vec![
        rect_command(rect(0.0, 0.0, 300.0, 300.0)),
        text_command(rect(0.0, 0.0, 100.0, 20.0)),
        rect_command(rect(200.0, 200.0, 50.0, 50.0)),
        text_command(rect(0.0, 30.0, 100.0, 20.0)),
        rect_command(rect(100.0, 0.0, 50.0, 20.0)),
        text_command(rect(100.0, 0.0, 50.0, 20.0)),
    ]);
    let batches = list.text_batches();
    assert_eq!(
        (batches.len(), batches.first().cloned()),
        (1, Some(1..6)),
        "a panel before the batch opened does not count; a rect touching the batch's edge misses it; text over a rect inside the range follows it in paint order"
    );
}

#[test]
fn a_non_text_command_over_the_batchs_text_closes_it() {
    let list = list_of(vec![
        text_command(rect(0.0, 0.0, 100.0, 20.0)),
        rect_command(rect(50.0, 10.0, 200.0, 100.0)),
        text_command(rect(60.0, 20.0, 100.0, 20.0)),
        text_command(rect(60.0, 40.0, 100.0, 20.0)),
        rect_command(rect(0.0, 0.0, 10.0, 10.0)),
        rect_command(rect(500.0, 500.0, 10.0, 10.0)),
        text_command(rect(0.0, 0.0, 5.0, 5.0)),
        rect_command(rect(0.0, 0.0, 300.0, 300.0)),
    ]);
    assert_eq!(list.text_batches(), [0..1, 2..7]);
    let list = list_of(vec![
        text_command(rect(0.0, 0.0, 100.0, 20.0)),
        text_command(rect(200.0, 200.0, 100.0, 20.0)),
        rect_command(rect(120.0, 100.0, 20.0, 20.0)),
        text_command(rect(0.0, 300.0, 10.0, 10.0)),
    ]);
    let batches = list.text_batches();
    assert_eq!(
        (batches.len(), batches.first().cloned()),
        (1, Some(0..4)),
        "a rect between two texts that touches neither keeps the batch open"
    );
}

#[test]
fn batches_cover_every_text_command_once_in_order() {
    let mut commands = Vec::new();
    for i in 0..12 {
        let y = (i % 4) as f32 * 25.0;
        commands.push(text_command(rect(0.0, y, 100.0, 20.0)));
        if i % 3 == 0 {
            commands.push(rect_command(rect(0.0, y, 100.0, 20.0)));
        }
    }
    let list = list_of(commands);
    let batches = list.text_batches();
    let mut seen = Vec::new();
    let mut last_end = 0;
    for batch in &batches {
        assert!(batch.start >= last_end, "batches are in order and disjoint");
        assert!(
            list.commands[batch.start].is_text(),
            "a batch opens at text"
        );
        assert!(list.commands[batch.end - 1].is_text(), "and ends at text");
        for i in batch.clone() {
            if list.commands[i].is_text() {
                seen.push(i);
            }
        }
        last_end = batch.end;
    }
    let texts: Vec<usize> = (0..list.commands.len())
        .filter(|i| list.commands[*i].is_text())
        .collect();
    assert_eq!(seen, texts);
    assert!(batches.len() > 1, "the overlaps split it");
    assert!(PaintList::default().text_batches().is_empty());
}

#[test]
fn an_image_paints_one_command_with_the_sprites_page_and_uv() {
    let (sprites, sprite) = registry("icon", None);
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let image = tree.image(
        root,
        sprite,
        Vec2::new(32.0, 32.0),
        at(8.0, 8.0, 64.0, 16.0),
    );
    tree.set_image_tint(image, [255, 0, 0, 128]);
    let list = paint(&mut tree, 2.0, UiDebug::default(), &sprites);
    assert_eq!(kinds(&list), "RI");
    assert_eq!(
        list.commands[1],
        PaintCommand::Image {
            rect: rect(16.0, 16.0, 128.0, 32.0),
            texture: TextureHandle(3),
            uv: SPRITE_UV,
            filter: FilterMode::Nearest,
            tint: [255, 0, 0, 128],
            clip: 0,
        }
    );
}

#[test]
fn a_nine_slice_keeps_its_corners_at_source_size_and_stretches_the_rest() {
    let (sprites, sprite) = registry("frame", Some([4, 4, 4, 4]));
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    tree.image(
        root,
        sprite,
        Vec2::new(32.0, 32.0),
        at(0.0, 0.0, 100.0, 50.0),
    );
    let list = paint(&mut tree, 2.0, UiDebug::default(), &sprites);
    assert_eq!(kinds(&list), "RIIIIIIIII");
    let slices: Vec<(Rect, UvRect)> = list.commands[1..]
        .iter()
        .map(|command| match command {
            PaintCommand::Image { rect, uv, .. } => (*rect, *uv),
            other => panic!("{other:?}"),
        })
        .collect();
    // Corners: 4 source px × scale 2 = 8 px; the UV inset is 4/32 of the
    // sprite's UV span (0.5 both ways).
    assert_eq!(
        slices[0],
        (
            rect(0.0, 0.0, 8.0, 8.0),
            UvRect {
                min: [0.25, 0.5],
                max: [0.3125, 0.5625]
            }
        )
    );
    assert_eq!(
        slices[4],
        (
            rect(8.0, 8.0, 184.0, 84.0),
            UvRect {
                min: [0.3125, 0.5625],
                max: [0.6875, 0.9375]
            }
        )
    );
    assert_eq!(
        slices[8],
        (
            rect(192.0, 92.0, 8.0, 8.0),
            UvRect {
                min: [0.6875, 0.9375],
                max: [0.75, 1.0]
            }
        )
    );
    assert_eq!(slices[1].0, rect(8.0, 0.0, 184.0, 8.0), "top edge");
    assert_eq!(slices[3].0, rect(0.0, 8.0, 8.0, 84.0), "left edge");
}

#[test]
fn a_box_smaller_than_its_corners_shrinks_them_and_skips_empty_slices() {
    let (sprites, sprite) = registry("frame", Some([4, 4, 4, 4]));
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    tree.image(root, sprite, Vec2::new(32.0, 32.0), at(0.0, 0.0, 4.0, 4.0));
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(
        kinds(&list),
        "RIIII",
        "four corners of 2 px, no edges or centre"
    );
    assert_eq!(list.commands[1].rect(), rect(0.0, 0.0, 2.0, 2.0));
    assert_eq!(list.commands[4].rect(), rect(2.0, 2.0, 2.0, 2.0));
}

#[test]
fn an_unregistered_sprite_draws_nothing_and_warns_once() {
    let mut sprites = AssetRegistry::new();
    let pending = sprites.intern_sprite("pending");
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    tree.image(root, pending, Vec2::new(32.0, 32.0), fixed(32.0, 32.0));
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(kinds(&list), "R");
    assert_eq!(tree.warned_sprites.len(), 1);
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(kinds(&list), "R");
    assert_eq!(
        tree.warned_sprites.len(),
        1,
        "the second paint warns again for no ID"
    );
}

#[test]
fn the_debug_flags_hide_the_game_layers_and_outline_every_painted_node() {
    let mut tree = UiTree::new();
    let game = tree.add_root(UiLayer::Game, LayoutStyle::default());
    tree.label(game, "hud");
    let popup = tree.add_root(UiLayer::GamePopup, at(10.0, 10.0, 100.0, 100.0));
    tree.button(popup, "ok");
    let debug = tree.add_root(UiLayer::Debug, at(0.0, 500.0, 300.0, 100.0));
    tree.label(debug, "fps");

    let all = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(kinds(&all), "RTRRTRT");

    let hidden = paint(
        &mut tree,
        1.0,
        UiDebug {
            hide_game_layers: true,
            show_bounds: false,
        },
        &AssetRegistry::new(),
    );
    assert_eq!(kinds(&hidden), "RT");
    assert_eq!(hidden.commands[0].rect(), rect(0.0, 500.0, 300.0, 100.0));

    let outlined = paint(
        &mut tree,
        1.0,
        UiDebug {
            hide_game_layers: false,
            show_bounds: true,
        },
        &AssetRegistry::new(),
    );
    assert_eq!(
        kinds(&outlined),
        "RTRRTRTRRRRRRR",
        "one outline per painted node"
    );
    assert_eq!(&outlined.commands[..7], &all.commands[..]);
    for outline in &outlined.commands[7..] {
        assert!(matches!(
            outline,
            PaintCommand::Rect {
                fill: [0, 0, 0, 0],
                border: BOUNDS_OUTLINE,
                border_width: 1.0,
                corner_radius: 0.0,
                clip: 0,
                ..
            }
        ));
    }
    assert_eq!(outlined.commands[7].rect(), rect(0.0, 0.0, 800.0, 20.0));

    let both = paint(
        &mut tree,
        1.0,
        UiDebug {
            hide_game_layers: true,
            show_bounds: true,
        },
        &AssetRegistry::new(),
    );
    assert_eq!(kinds(&both), "RTRR", "hidden layers get no outline");
}

#[test]
fn the_dump_is_the_snapshot_format() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    tree.label(root, "hi");
    tree.button(root, "ok");
    let mut store = FixedAdvanceMeasure::default();
    let list = paint_with(
        &mut tree,
        &mut store,
        1.0,
        UiDebug::default(),
        &AssetRegistry::new(),
    );
    assert_eq!(
        list.dump(),
        "clip 0 (0, 0) 800×600\n\
         Rect (0.0, 0.0) 800.0×40.0 fill #202128ff border #464854ff 1.0 r 4.0 clip 0\n\
         -- batch 0\n\
         Text node 0:0 (0.0, 0.0) 800.0×20.0 #ebebf0ff clip 0\n\
         Rect (0.0, 20.0) 800.0×20.0 fill #569cffff border #464854ff 1.0 r 4.0 clip 0\n\
         Text node 1:0 (0.0, 20.0) 800.0×20.0 #ffffffff clip 0\n"
    );

    let (sprites, sprite) = registry("icon", None);
    tree.image(root, sprite, Vec2::new(32.0, 32.0), fixed(32.0, 32.0));
    let list = paint_with(&mut tree, &mut store, 1.0, UiDebug::default(), &sprites);
    assert!(
        list.dump().ends_with(
            "Image (0.0, 40.0) 32.0×32.0 tex 3 uv (0.250, 0.500)-(0.750, 1.000) nearest tint #ffffffff clip 0\n"
        ),
        "{}",
        list.dump()
    );
    assert_eq!(PaintList::default().dump(), "");
}

#[test]
fn an_empty_tree_paints_the_viewport_clip_and_nothing_else() {
    let mut tree = UiTree::new();
    let mut list = PaintList::default();
    tree.paint(1.0, &UiDebug::default(), &AssetRegistry::new(), &mut list);
    assert_eq!(list.clips, [ClipRect::default()], "no layout, no viewport");
    assert!(list.is_empty());

    let list = paint(&mut tree, 2.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(
        list.clips,
        [ClipRect {
            x: 0,
            y: 0,
            width: 1600,
            height: 1200
        }]
    );
    assert!(list.is_empty());
    assert!(list.text_batches().is_empty());

    let mut reused = list;
    reused.commands.push(rect_command(rect(0.0, 0.0, 1.0, 1.0)));
    reused.clear();
    assert!(reused.is_empty() && reused.clips.is_empty());
    assert_eq!(reused.scale, 2.0, "clear keeps the scale");
}
