use std::path::PathBuf;

use glam::Vec2;

use super::*;
use crate::assets::{AssetRegistry, SpriteAssetId};
use crate::text::TextNodeStore as _;
use crate::ui::theme::{StyleOverrides, WidgetState};
use crate::ui::{
    Anchor, FixedAdvanceMeasure, LayoutStats, LayoutStyle, Sizing, SliderRange, UiVertex,
    Visibility,
};

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
        scale: 1.0,
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
        vertices: Vec::new(),
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
            PaintCommand::Mesh { .. } => "M",
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

#[test]
fn physical_rects_far_from_the_origin_do_not_overflow() {
    // A box past about -2.1e9 physical px: `right - x` overflowed `i32`,
    // panicking a debug build and wrapping to an empty clip in release.
    let far = Rect::new(Vec2::new(-3e9, -3e9), Vec2::new(100.0, 50.0));
    let clip = to_physical(far, 1.0);
    assert_eq!((clip.x, clip.y), (-(1 << 30), -(1 << 30)));
    assert_eq!(clip.x + clip.width, 100);
    assert_eq!(clip.y + clip.height, 50);
    let viewport = ClipRect {
        x: 0,
        y: 0,
        width: 800,
        height: 600,
    };
    assert_eq!(
        clip.intersect(viewport),
        ClipRect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        }
    );
    let past = to_physical(Rect::new(Vec2::splat(-1.0), Vec2::splat(1e30)), 2.0);
    assert_eq!((past.x, past.width), (-2, (1 << 30) + 1));
    // Edge sums of rects built by hand stay in range too.
    let wide = ClipRect {
        x: i32::MAX - 10,
        y: 0,
        width: i32::MAX,
        height: 10,
    };
    assert_eq!(wide.intersect(wide).width, i32::MAX);
    assert_eq!(wide.intersect(viewport).width, 0);
}

/// A root at the origin with no fill or border, so only its children paint.
fn bare_root(tree: &mut UiTree, style: LayoutStyle) -> WidgetId {
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_overrides(
        root,
        StyleOverrides {
            background: Some([0, 0, 0, 0]),
            border_width: Some(0.0),
            ..StyleOverrides::default()
        },
    );
    root
}

/// The dump of a lone slider of `width`×`height` at `value` in 0 to 10 by 1.
fn slider_dump(width: f32, height: f32, value: f32, scale: f32) -> String {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let slider = tree.slider(root, SliderRange::new(0.0, 10.0).with_step(1.0), value);
    tree.set_layout_style(slider, fixed(width, height));
    paint(&mut tree, scale, UiDebug::default(), &AssetRegistry::new()).dump()
}

#[test]
fn a_toggle_paints_its_box_and_its_mark_only_when_on() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let toggle = tree.toggle(root, false);
    tree.set_layout_style(toggle, fixed(20.0, 20.0));
    let sprites = AssetRegistry::new();
    let off = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(
        off.dump(),
        "clip 0 (0, 0) 800×600\n\
         Rect (0.0, 0.0) 20.0×20.0 fill #202128ff border #464854ff 1.0 r 2.0 clip 0\n"
    );

    tree.set_toggled(toggle, true);
    let on = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(
        on.dump(),
        "clip 0 (0, 0) 800×600\n\
         Rect (0.0, 0.0) 20.0×20.0 fill #202128ff border #464854ff 1.0 r 2.0 clip 0\n\
         Rect (5.0, 5.0) 10.0×10.0 fill #569cffff border #00000000 0.0 r 0.0 clip 0\n",
        "the mark: inset a quarter of the shorter side, in the text colour"
    );

    // A wider box with rounder corners: the inset follows the shorter side,
    // the mark's radius the box's less the inset; scaled once.
    tree.set_layout_style(toggle, fixed(40.0, 24.0));
    tree.set_overrides(
        toggle,
        StyleOverrides {
            corner_radius: Some(8.0),
            ..StyleOverrides::default()
        },
    );
    let wide = paint(&mut tree, 1.5, UiDebug::default(), &sprites);
    assert_eq!(
        wide.dump(),
        "clip 0 (0, 0) 1200×900\n\
         Rect (0.0, 0.0) 60.0×36.0 fill #202128ff border #464854ff 1.5 r 12.0 clip 0\n\
         Rect (9.0, 9.0) 42.0×18.0 fill #569cffff border #00000000 0.0 r 3.0 clip 0\n"
    );
}

#[test]
fn a_slider_paints_its_track_fill_and_thumb_in_place_of_its_box() {
    // §2.6's example: 200×20 halfway.
    assert_eq!(
        slider_dump(200.0, 20.0, 5.0, 1.0),
        "clip 0 (0, 0) 800×600\n\
         Rect (0.0, 7.5) 200.0×5.0 fill #464854ff border #00000000 0.0 r 2.0 clip 0\n\
         Rect (0.0, 7.5) 100.0×5.0 fill #569cffff border #00000000 0.0 r 2.0 clip 0\n\
         Rect (90.0, 0.0) 20.0×20.0 fill #569cffff border #464854ff 0.0 r 2.0 clip 0\n"
    );
    // 200×24 at its minimum, a middle step and its maximum, at scales 1
    // and 1.5: the thumb's centre runs from 12 to 188.
    let cases = [
        (
            0.0,
            1.0,
            "(0.0, 9.0) 200.0×6.0",
            "(0.0, 9.0) 12.0×6.0",
            "(0.0, 0.0) 24.0×24.0",
            "2.0",
        ),
        (
            5.0,
            1.0,
            "(0.0, 9.0) 200.0×6.0",
            "(0.0, 9.0) 100.0×6.0",
            "(88.0, 0.0) 24.0×24.0",
            "2.0",
        ),
        (
            10.0,
            1.0,
            "(0.0, 9.0) 200.0×6.0",
            "(0.0, 9.0) 188.0×6.0",
            "(176.0, 0.0) 24.0×24.0",
            "2.0",
        ),
        (
            0.0,
            1.5,
            "(0.0, 13.5) 300.0×9.0",
            "(0.0, 13.5) 18.0×9.0",
            "(0.0, 0.0) 36.0×36.0",
            "3.0",
        ),
        (
            5.0,
            1.5,
            "(0.0, 13.5) 300.0×9.0",
            "(0.0, 13.5) 150.0×9.0",
            "(132.0, 0.0) 36.0×36.0",
            "3.0",
        ),
        (
            10.0,
            1.5,
            "(0.0, 13.5) 300.0×9.0",
            "(0.0, 13.5) 282.0×9.0",
            "(264.0, 0.0) 36.0×36.0",
            "3.0",
        ),
    ];
    for (value, scale, track, fill, thumb, r) in cases {
        let clip = if scale == 1.0 {
            "800×600"
        } else {
            "1200×900"
        };
        assert_eq!(
            slider_dump(200.0, 24.0, value, scale),
            format!(
                "clip 0 (0, 0) {clip}\n\
                 Rect {track} fill #464854ff border #00000000 0.0 r {r} clip 0\n\
                 Rect {fill} fill #569cffff border #00000000 0.0 r {r} clip 0\n\
                 Rect {thumb} fill #569cffff border #464854ff 0.0 r {r} clip 0\n"
            ),
            "value {value} at scale {scale}"
        );
    }
}

#[test]
fn a_singleton_sliders_thumb_sits_at_the_start() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let slider = tree.slider(root, SliderRange::new(3.0, 3.0), 3.0);
    tree.set_layout_style(slider, fixed(200.0, 20.0));
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(
        rects(&list)[1..],
        [rect(0.0, 7.5, 10.0, 5.0), rect(0.0, 0.0, 20.0, 20.0)],
        "the fill to the thumb's centre, the thumb at the start"
    );
}

#[test]
fn focus_rings_the_thumb_and_disabled_controls_take_the_disabled_colour() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let slider = tree.slider(root, SliderRange::new(0.0, 1.0), 0.5);
    tree.set_layout_style(slider, fixed(200.0, 20.0));
    assert!(tree.focus(slider));
    let sprites = AssetRegistry::new();
    let focused = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(
        focused.dump(),
        "clip 0 (0, 0) 800×600\n\
         Rect (0.0, 7.5) 200.0×5.0 fill #464854ff border #00000000 0.0 r 2.0 clip 0\n\
         Rect (0.0, 7.5) 100.0×5.0 fill #569cffff border #00000000 0.0 r 2.0 clip 0\n\
         Rect (90.0, 0.0) 20.0×20.0 fill #569cffff border #ffc440ff 2.0 r 2.0 clip 0\n",
        "the focus border rings the thumb, not the track"
    );

    let toggle = tree.toggle(root, true);
    tree.set_layout_style(toggle, fixed(20.0, 20.0));
    tree.set_enabled(slider, false);
    tree.set_enabled(toggle, false);
    let disabled = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(
        disabled.dump(),
        "clip 0 (0, 0) 800×600\n\
         Rect (0.0, 7.5) 200.0×5.0 fill #464854ff border #00000000 0.0 r 2.0 clip 0\n\
         Rect (0.0, 7.5) 100.0×5.0 fill #787a87ff border #00000000 0.0 r 2.0 clip 0\n\
         Rect (90.0, 0.0) 20.0×20.0 fill #787a87ff border #464854ff 0.0 r 2.0 clip 0\n\
         Rect (0.0, 20.0) 20.0×20.0 fill #202128ff border #464854ff 1.0 r 2.0 clip 0\n\
         Rect (5.0, 25.0) 10.0×10.0 fill #787a87ff border #00000000 0.0 r 0.0 clip 0\n"
    );
}

#[test]
fn a_row_of_labels_and_controls_keeps_its_text_in_one_batch() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::row());
    tree.label(root, "On");
    tree.toggle(root, true);
    tree.label(root, "Vol");
    tree.slider(root, SliderRange::new(0.0, 1.0), 0.5);
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(kinds(&list), "TRRTRRR");
    assert_eq!(
        rects(&list),
        [
            rect(0.0, 0.0, 16.0, 20.0),
            rect(16.0, 0.0, 20.0, 20.0),
            rect(21.0, 5.0, 10.0, 10.0),
            rect(36.0, 0.0, 24.0, 20.0),
            rect(60.0, 7.5, 200.0, 5.0),
            rect(60.0, 7.5, 100.0, 5.0),
            rect(150.0, 0.0, 20.0, 20.0),
        ]
    );
    let batches = list.text_batches();
    assert_eq!(
        (batches.len(), batches.first().cloned()),
        (1, Some(0..4)),
        "a control's rectangles overlap no label beside it"
    );
}

// ---- M56: paint-only properties ----

/// Every colour's alpha a command paints with: fill and border, tint, text.
fn alphas(list: &PaintList) -> Vec<u8> {
    list.commands
        .iter()
        .flat_map(|command| match command {
            PaintCommand::Rect { fill, border, .. } => vec![fill[3], border[3]],
            PaintCommand::Image { tint, .. } => vec![tint[3]],
            PaintCommand::Text { color, .. } => vec![color[3]],
            PaintCommand::Mesh { .. } => list
                .mesh_vertices(command)
                .iter()
                .map(|vertex| vertex.color[3])
                .collect(),
        })
        .collect()
}

fn opaque() -> StyleOverrides {
    StyleOverrides {
        background: Some(WHITE),
        border: Some(WHITE),
        border_width: Some(2.0),
        corner_radius: Some(4.0),
        ..StyleOverrides::default()
    }
}

/// A root holding a filled panel at (100, 100), 100 by 50, which holds a
/// label "ab" and an image: (tree, root, panel, label, image, sprites).
fn painted_panel() -> (
    UiTree,
    WidgetId,
    WidgetId,
    WidgetId,
    WidgetId,
    AssetRegistry,
) {
    let (sprites, sprite) = registry("icon", None);
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let panel = tree.panel(root, at(100.0, 100.0, 100.0, 50.0));
    tree.set_overrides(panel, opaque());
    let label = tree.label(panel, "ab");
    let image = tree.image(panel, sprite, Vec2::new(10.0, 10.0), LayoutStyle::default());
    (tree, root, panel, label, image, sprites)
}

#[test]
fn opacity_and_a_roots_fade_multiply_every_commands_alpha_in_the_subtree() {
    let (mut tree, root, panel, _label, _image, sprites) = painted_panel();
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(kinds(&list), "RTI");
    assert!(alphas(&list).iter().all(|alpha| *alpha == 255));

    tree.set_fade(root, 0.5);
    tree.set_opacity(panel, 0.5);
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty(), "paint only");
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(kinds(&list), "RTI");
    assert!(
        alphas(&list).iter().all(|alpha| *alpha == 64),
        "255 × 0.25, rounded: {:?}",
        alphas(&list)
    );
}

#[test]
fn an_alpha_zero_subtree_paints_nothing_and_is_still_hit() {
    let (mut tree, _root, panel, _label, _image, sprites) = painted_panel();
    tree.set_opacity(panel, 0.0);
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert!(list.is_empty());
    assert_eq!(tree.hit_test(Vec2::new(195.0, 145.0)), Some(panel));
}

#[test]
fn a_translation_moves_paint_and_hits_by_whole_physical_pixels() {
    let (mut tree, _root, panel, _label, _image, sprites) = painted_panel();
    let before = paint(&mut tree, 1.5, UiDebug::default(), &sprites);
    tree.set_translation(panel, Vec2::new(3.3, -0.2));
    assert!(!tree.is_layout_dirty(), "paint only");
    let after = paint(&mut tree, 1.5, UiDebug::default(), &sprites);
    for (old, new) in rects(&before).iter().zip(rects(&after)) {
        // 3.3 × 1.5 = 4.95 rounds to 5 physical pixels; -0.3 to 0.
        assert_eq!(new.min - old.min, Vec2::new(5.0, 0.0));
        assert_eq!(new.size(), old.size());
    }
    let shift = 5.0 / 1.5;
    let visual = tree.visual_rect(panel).unwrap();
    assert_eq!(visual.min, Vec2::new(100.0 + shift, 100.0));
    assert_eq!(tree.rect(panel), Some(rect(100.0, 100.0, 100.0, 50.0)));
    // Past the layout box's right edge but inside the painted one, below
    // the image.
    let right = Vec2::new(200.0 + shift * 0.5, 140.0);
    assert_eq!(tree.hit_test(right), Some(panel));
    assert_eq!(tree.hit_test(Vec2::new(100.0 + shift * 0.5, 140.0)), None);
}

#[test]
fn a_scale_maps_rects_radii_borders_clips_and_text_about_the_centre() {
    let (mut tree, _root, panel, label, _image, sprites) = painted_panel();
    tree.set_clip_children(panel, true);
    let before = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    tree.set_scale(panel, 2.0);
    let after = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    let map = |p: Vec2| Vec2::new(150.0, 125.0) + (p - Vec2::new(150.0, 125.0)) * 2.0;
    for (old, new) in rects(&before).iter().zip(rects(&after)) {
        assert_eq!(new.min, map(old.min));
        assert_eq!(new.max, map(old.max));
    }
    let PaintCommand::Rect {
        border_width,
        corner_radius,
        ..
    } = after.commands[0]
    else {
        panic!("the panel's box first");
    };
    assert_eq!((border_width, corner_radius), (4.0, 8.0));
    assert_eq!(
        after.clips[1],
        to_physical(rect(50.0, 75.0, 200.0, 100.0), 1.0),
        "the panel's clip is its scaled box"
    );
    let PaintCommand::Text { scale, rect, .. } = after.commands[1] else {
        panic!("the label's run second");
    };
    assert_eq!(scale, 2.0);
    let label_box = tree.rect(label).unwrap();
    assert_eq!(rect.min, map(label_box.min));
    assert_eq!(rect.max, map(label_box.max));
}

#[test]
fn text_scales_round_down_to_a_32nd_inside_their_exact_box() {
    for (set, drawn) in [
        (1.02, 1.0),
        (1.03125, 1.03125),
        (1.0 / 40.0, 1.0 / 40.0),
        (0.5, 0.5),
        (3.0, 3.0),
    ] {
        let (mut tree, _root, panel, label, _image, sprites) = painted_panel();
        tree.set_scale(panel, set);
        let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
        let PaintCommand::Text { scale, rect, .. } = list.commands[1] else {
            panic!("the label's run second");
        };
        assert_eq!(scale, drawn, "set {set}");
        let exact = tree.visual_rect(label).unwrap();
        assert!(
            rect.min.x >= exact.min.x - 1e-4
                && rect.min.y >= exact.min.y - 1e-4
                && rect.max.x <= exact.max.x + 1e-4
                && rect.max.y <= exact.max.y + 1e-4,
            "set {set}: {rect:?} inside {exact:?}"
        );
        let centre = |r: Rect| (r.min + r.max) * 0.5;
        assert!((centre(rect) - centre(exact)).length() < 1e-3);
        assert!((rect.width() - exact.width() * drawn / set).abs() < 1e-3);
    }
    assert_eq!(text_scale_step(1.0 / 32.0), 1.0 / 32.0);
    assert_eq!(text_scale_step(0.03), 0.03);
}

#[test]
fn nested_scales_paint_down_to_a_64th_and_below_it_nothing_paints_or_hits() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let outer = tree.panel(root, at(0.0, 0.0, 800.0, 600.0));
    tree.set_overrides(
        outer,
        StyleOverrides {
            background: Some([0; 4]),
            border_width: Some(0.0),
            ..StyleOverrides::default()
        },
    );
    let inner = tree.panel(outer, at(0.0, 0.0, 800.0, 600.0));
    tree.set_overrides(inner, opaque());
    tree.label(inner, "ab");
    tree.set_scale(outer, 1.0 / 8.0);
    tree.set_scale(inner, 1.0 / 8.0);
    let sprites = AssetRegistry::new();
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(kinds(&list), "RT", "1/8 × 1/8 = 1/64 paints");
    assert_eq!(tree.hit_test(Vec2::new(400.0, 300.0)), Some(inner));

    tree.set_scale(inner, 1.0 / 16.0);
    let list = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert!(list.is_empty(), "1/128 paints nothing");
    assert_eq!(
        tree.hit_test(Vec2::new(400.0, 300.0)),
        Some(outer),
        "and hits nothing under it"
    );
}

#[test]
fn paint_only_setters_ignore_what_is_not_finite_and_clamp_the_rest() {
    let (mut tree, root, panel, _label, _image, sprites) = painted_panel();
    paint(&mut tree, 1.0, UiDebug::default(), &sprites);

    tree.set_opacity(panel, f32::NAN);
    tree.set_translation(panel, Vec2::new(f32::NAN, 0.0));
    tree.set_translation(panel, Vec2::new(0.0, f32::INFINITY));
    for scale in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
        tree.set_scale(panel, scale);
    }
    tree.set_fade(root, f32::NAN);
    assert!(!tree.is_paint_dirty(), "every one ignored");
    assert_eq!(tree.opacity(panel), Some(1.0));
    assert_eq!(tree.translation(panel), Some(Vec2::ZERO));
    assert_eq!(tree.scale(panel), Some(1.0));
    assert_eq!(tree.fade(root), Some(1.0));

    tree.set_opacity(panel, -1.0);
    assert_eq!(tree.opacity(panel), Some(0.0));
    tree.set_opacity(panel, f32::INFINITY);
    assert_eq!(tree.opacity(panel), Some(1.0));
    tree.set_scale(panel, 1e9);
    assert_eq!(tree.scale(panel), Some(64.0));
    tree.set_scale(panel, 1e-9);
    assert_eq!(tree.scale(panel), Some(1.0 / 64.0));
    tree.set_fade(root, -2.0);
    assert_eq!(tree.fade(root), Some(0.0));
    tree.set_fade(root, f32::INFINITY);
    assert_eq!(tree.fade(root), Some(1.0));
    tree.set_translation(panel, Vec2::new(-4.0, 0.0));
    assert_eq!(tree.translation(panel), Some(Vec2::new(-4.0, 0.0)));
    assert!(!tree.is_layout_dirty(), "never layout");

    tree.clear_paint_dirty();
    tree.set_translation(panel, Vec2::new(-4.0, 0.0));
    tree.set_fade(root, 1.0);
    assert!(!tree.is_paint_dirty(), "an equal value marks nothing");

    // A widget that is not a root takes no fade or freeze.
    tree.set_fade(panel, 0.5);
    tree.set_frozen(panel, true);
    assert_eq!(tree.fade(panel), None);
    assert!(!tree.frozen(panel));
    tree.set_frozen(root, true);
    assert!(tree.frozen(root));
    assert!(!tree.is_paint_dirty(), "a freeze marks nothing");
    let gone = WidgetId::new(99, 0);
    assert_eq!(tree.opacity(gone), None);
    assert_eq!(tree.translation(gone), None);
    assert_eq!(tree.scale(gone), None);
}

#[test]
fn a_padded_label_commits_and_paints_at_its_content_box() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    // "aaa bbb" is 56 wide: one line in the 60 border box, two in the 40
    // content box.
    let label = tree.label(root, "aaa bbb");
    tree.set_layout_style(
        label,
        LayoutStyle::default()
            .with_size(Sizing::Px(60.0), Sizing::Auto)
            .with_padding(10.0),
    );
    let mut store = FixedAdvanceMeasure::default();
    let list = paint_with(
        &mut tree,
        &mut store,
        1.0,
        UiDebug::default(),
        &AssetRegistry::new(),
    );
    let border = tree.rect(label).unwrap();
    assert_eq!(border.size(), Vec2::new(60.0, 60.0));
    let committed = store.committed(tree.text_node(label).unwrap()).unwrap();
    assert_eq!(committed.size.y, 40.0, "two lines in the content box");
    let PaintCommand::Text { rect: text, .. } = list.commands[0] else {
        panic!("one text run");
    };
    assert_eq!(text.min, border.min + Vec2::splat(10.0));
    assert_eq!(text.size(), Vec2::new(40.0, 40.0));
}

#[test]
fn a_labels_colour_only_change_repaints_without_setting_its_text() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let label = tree.label(root, "ab");
    let mut store = FixedAdvanceMeasure::default();
    let sprites = AssetRegistry::new();
    paint_with(&mut tree, &mut store, 1.0, UiDebug::default(), &sprites);
    let before = store.counts;
    let mut overrides = StyleOverrides::default();
    overrides.text.color = Some([1, 2, 3, 255]);
    tree.set_overrides(label, overrides);
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats, LayoutStats::default(), "no relayout");
    assert!(tree.is_paint_dirty());
    assert_eq!(store.counts.set_text_calls, before.set_text_calls);
    let list = paint_with(&mut tree, &mut store, 1.0, UiDebug::default(), &sprites);
    let PaintCommand::Text { color, .. } = list.commands[0] else {
        panic!("one text run");
    };
    assert_eq!(color, [1, 2, 3, 255]);
}

#[test]
fn a_widget_under_a_disabled_panel_resolves_disabled_and_back() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let panel = tree.panel(root, LayoutStyle::default());
    let button = tree.button(panel, "go");
    let sprites = AssetRegistry::new();
    paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    let normal = tree.resolved_style(button).unwrap().background;
    tree.set_enabled(panel, false);
    paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(tree.widget_state(button), Some(WidgetState::Disabled));
    let theme = tree.theme().clone();
    assert_eq!(
        tree.resolved_style(button).unwrap().background,
        theme.kinds.button.disabled.background.unwrap()
    );
    assert!(tree.enabled(button), "its own flag stays");
    tree.set_enabled(panel, true);
    paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert_eq!(tree.widget_state(button), Some(WidgetState::Normal));
    assert_eq!(tree.resolved_style(button).unwrap().background, normal);
}

// ---- M56: prewarm ----

#[test]
fn a_prewarm_paints_a_hidden_roots_runs_once_at_their_transforms() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let panel = tree.panel(root, at(100.0, 100.0, 100.0, 50.0));
    let label = tree.label(panel, "ab");
    let digits = tree.label(panel, "0123456789");
    tree.set_visibility(digits, Visibility::Hidden);
    tree.set_visibility(root, Visibility::Hidden);
    tree.set_scale(panel, 2.0);
    tree.set_opacity(panel, 0.0);
    tree.prewarm(root);
    tree.prewarm(panel);
    assert!(tree.prewarm_pending());

    let mut list = PaintList::default();
    assert!(!tree.paint_prewarm(1.0, &mut list), "not laid out yet");
    assert!(
        tree.prewarm_pending(),
        "the request waits for the first layout"
    );

    let sprites = AssetRegistry::new();
    let shown = paint(&mut tree, 1.0, UiDebug::default(), &sprites);
    assert!(shown.is_empty(), "hidden: the frame paints nothing");
    assert!(tree.paint_prewarm(1.5, &mut list));
    assert_eq!(kinds(&list), "TT", "both runs, the hidden one too");
    assert_eq!(list.scale, 1.5);
    assert_eq!(list.clips.len(), 1);
    let PaintCommand::Text {
        scale, rect, node, ..
    } = list.commands[0]
    else {
        panic!("a run");
    };
    assert_eq!(node, tree.text_node(label).unwrap());
    assert_eq!(scale, 2.0, "at its transform");
    let visual = tree.visual_rect(label).unwrap();
    assert_eq!(rect.min, visual.min * 1.5);
    assert!(!tree.prewarm_pending());
    assert!(!tree.paint_prewarm(1.5, &mut list));
    assert!(list.is_empty(), "nothing on the next call");
}

#[test]
fn a_collapsed_or_removed_root_drops_its_prewarm() {
    let mut tree = UiTree::new();
    let collapsed = bare_root(&mut tree, LayoutStyle::default());
    tree.label(collapsed, "x");
    tree.set_visibility(collapsed, Visibility::Collapsed);
    let removed = bare_root(&mut tree, LayoutStyle::default());
    tree.label(removed, "y");
    tree.prewarm(collapsed);
    tree.prewarm(removed);
    tree.remove(removed);
    paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    let mut list = PaintList::default();
    assert!(!tree.paint_prewarm(1.0, &mut list));
    assert!(!tree.prewarm_pending(), "both requests dropped");
}

// ---- M57: meshes ----

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 200];

fn vertex(x: f32, y: f32, color: [u8; 4]) -> UiVertex {
    UiVertex::new(Vec2::new(x, y), color)
}

/// No fill and no border, so a widget paints only what it holds.
fn bare() -> StyleOverrides {
    StyleOverrides {
        background: Some([0, 0, 0, 0]),
        border_width: Some(0.0),
        ..StyleOverrides::default()
    }
}

/// A bare mesh under a bare root, laid out by `style`.
fn bare_mesh(tree: &mut UiTree, style: LayoutStyle) -> (WidgetId, WidgetId) {
    let root = bare_root(tree, LayoutStyle::default());
    let mesh = tree.mesh(root, style);
    tree.set_overrides(mesh, bare());
    (root, mesh)
}

#[test]
fn a_meshs_vertices_map_into_its_box_under_its_transform_with_the_fade() {
    let mut tree = UiTree::new();
    let (root, mesh) = bare_mesh(&mut tree, at(100.0, 100.0, 100.0, 50.0));
    let triangle = [
        vertex(0.0, 0.0, RED),
        vertex(1.0, 0.0, GREEN),
        vertex(1.0, 1.0, BLUE),
    ];
    assert!(tree.set_mesh(mesh, &triangle));
    assert_eq!(tree.kind(mesh), Some(WidgetKind::Mesh));
    assert_eq!(tree.role(mesh), Some(crate::ui::Role::Image));
    tree.set_scale(mesh, 2.0);
    tree.set_translation(mesh, Vec2::new(10.0, 0.0));
    tree.set_fade(root, 0.5);
    let list = paint(&mut tree, 1.5, UiDebug::default(), &AssetRegistry::new());

    assert_eq!(kinds(&list), "M", "a bare box paints nothing of its own");
    // p ↦ (150, 125) + 2 · (p − (150, 125)) + (10, 0), then × 1.5.
    let expected = [
        vertex(90.0, 112.5, [255, 0, 0, 128]),
        vertex(390.0, 112.5, [0, 255, 0, 128]),
        vertex(390.0, 262.5, [0, 0, 255, 100]),
    ];
    assert_eq!(list.mesh_vertices(&list.commands[0]), expected);
    assert_eq!(list.vertices, expected);
    assert_eq!(
        list.commands[0],
        PaintCommand::Mesh {
            rect: Rect::new(Vec2::new(90.0, 112.5), Vec2::new(390.0, 262.5)),
            first: 0,
            count: 3,
            clip: 0,
        }
    );
    assert!(
        list.mesh_vertices(&rect_command(rect(0.0, 0.0, 1.0, 1.0)))
            .is_empty()
    );
    let mut reused = list;
    reused.clear();
    assert!(reused.vertices.is_empty(), "clear drops the vertices");
}

#[test]
fn set_mesh_keeps_whole_finite_triangles_and_marks_paint_only() {
    let mut tree = UiTree::new();
    let (root, mesh) = bare_mesh(&mut tree, at(0.0, 0.0, 10.0, 10.0));
    let label = tree.label(root, "x");
    paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert!(!tree.is_layout_dirty() && !tree.is_paint_dirty());

    let a = [
        vertex(0.0, 0.0, RED),
        vertex(1.0, 0.0, RED),
        vertex(0.0, 1.0, RED),
    ];
    let d = [
        vertex(0.5, 0.5, BLUE),
        vertex(2.0, 0.5, BLUE),
        vertex(0.5, -1.0, BLUE),
    ];
    let mut vertices = a.to_vec();
    vertices.extend([
        vertex(f32::NAN, 0.0, GREEN),
        vertex(1.0, 0.0, GREEN),
        vertex(0.0, 1.0, GREEN),
    ]);
    vertices.extend([
        vertex(0.0, 0.0, GREEN),
        vertex(1.0, f32::INFINITY, GREEN),
        vertex(0.0, 1.0, GREEN),
    ]);
    vertices.extend(d);
    vertices.extend([vertex(0.0, 0.0, RED), vertex(1.0, 1.0, RED)]);
    assert!(tree.set_mesh(mesh, &vertices));
    let kept: Vec<UiVertex> = a.iter().chain(&d).copied().collect();
    assert_eq!(tree.mesh_vertices(mesh), kept);
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty(), "paint only");

    paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert!(!tree.set_mesh(mesh, &vertices), "the same triangles");
    assert!(!tree.set_mesh(mesh, &kept), "the same kept triangles");
    assert!(!tree.is_paint_dirty());
    assert!(tree.set_mesh(mesh, &[]));
    assert!(tree.mesh_vertices(mesh).is_empty());

    assert!(!tree.set_mesh(label, &a), "not a mesh");
    assert!(tree.mesh_vertices(label).is_empty());
}

#[test]
fn a_mesh_with_no_triangle_paints_only_its_box_and_measures_zero() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let mesh = tree.mesh(
        root,
        LayoutStyle::default().with_size(Sizing::Px(40.0), Sizing::Auto),
    );
    tree.set_mesh(mesh, &[vertex(0.0, 0.0, RED), vertex(1.0, 1.0, RED)]);
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(
        tree.rect(mesh),
        Some(rect(0.0, 0.0, 40.0, 0.0)),
        "no natural size"
    );
    assert!(list.vertices.is_empty());
    assert!(
        list.commands
            .iter()
            .all(|command| !matches!(command, PaintCommand::Mesh { .. }))
    );

    tree.set_layout_style(mesh, fixed(40.0, 30.0));
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(
        list.dump(),
        "clip 0 (0, 0) 800×600\n\
         Rect (0.0, 0.0) 40.0×30.0 fill #202128ff border #464854ff 1.0 r 4.0 clip 0\n",
        "the panel's look, and no triangle"
    );
}

#[test]
fn the_dump_line_of_a_mesh() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, at(0.0, 0.0, 200.0, 200.0));
    tree.set_clip_children(root, true);
    let mesh = tree.mesh(root, at(10.0, 20.0, 40.0, 30.0));
    tree.set_overrides(mesh, bare());
    tree.set_mesh(
        mesh,
        &[
            vertex(0.0, 0.0, RED),
            vertex(1.0, 0.0, RED),
            vertex(0.0, 1.0, RED),
            vertex(1.0, 0.0, GREEN),
            vertex(1.0, 1.0, GREEN),
            vertex(0.0, 1.0, GREEN),
        ],
    );
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());
    assert_eq!(
        list.dump(),
        "clip 0 (0, 0) 800×600\n\
         clip 1 (0, 0) 200×200\n\
         Mesh (10.0, 20.0) 40.0×30.0 verts 6 clip 1\n"
    );
}

#[test]
fn a_mesh_over_text_closes_its_batch() {
    let mut list = list_of(vec![
        text_command(rect(0.0, 0.0, 100.0, 20.0)),
        PaintCommand::Mesh {
            rect: rect(50.0, 10.0, 100.0, 100.0),
            first: 0,
            count: 3,
            clip: 0,
        },
        text_command(rect(60.0, 20.0, 100.0, 20.0)),
    ]);
    list.vertices = vec![
        vertex(50.0, 10.0, RED),
        vertex(150.0, 10.0, RED),
        vertex(50.0, 110.0, RED),
    ];
    assert_eq!(list.text_batches(), [0..1, 2..3]);
    let missed = list_of(vec![
        text_command(rect(0.0, 0.0, 100.0, 20.0)),
        PaintCommand::Mesh {
            rect: rect(0.0, 20.0, 100.0, 100.0),
            first: 0,
            count: 3,
            clip: 0,
        },
        text_command(rect(0.0, 200.0, 100.0, 20.0)),
    ]);
    let batches = missed.text_batches();
    assert_eq!(
        (batches.len(), batches.first().cloned()),
        (1, Some(0..3)),
        "a mesh touching the text's edge misses it"
    );
}

#[test]
fn a_triangle_leaving_its_box_over_earlier_text_closes_that_batch() {
    let mut tree = UiTree::new();
    let root = bare_root(&mut tree, LayoutStyle::default());
    let a = tree.label(root, "aaaa");
    tree.set_layout_style(a, at(0.0, 0.0, 100.0, 20.0));
    let mesh = tree.mesh(root, at(0.0, 100.0, 50.0, 50.0));
    tree.set_overrides(mesh, bare());
    // The apex sits three box heights above the box: y = 100 − 150.
    tree.set_mesh(
        mesh,
        &[
            vertex(0.0, 0.0, RED),
            vertex(1.0, 0.0, RED),
            vertex(0.5, -3.0, RED),
        ],
    );
    let b = tree.label(root, "bbbb");
    tree.set_layout_style(b, at(300.0, 300.0, 100.0, 20.0));
    let list = paint(&mut tree, 1.0, UiDebug::default(), &AssetRegistry::new());

    assert_eq!(kinds(&list), "TMT");
    assert_eq!(tree.rect(mesh), Some(rect(0.0, 100.0, 50.0, 50.0)));
    assert_eq!(
        list.commands[1].rect(),
        Rect::new(Vec2::new(0.0, -50.0), Vec2::new(50.0, 100.0)),
        "the command bounds the triangle, not the box"
    );
    assert_eq!(
        list.text_batches(),
        [0..1, 2..3],
        "A's batch closes before the mesh, which draws over A"
    );
}
