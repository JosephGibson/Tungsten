//! The Taffy side of layout (`D-124`): the `LayoutStyle` conversion, one
//! solver tree per dirty root inside a synthetic viewport-sized parent, the
//! measure closure over [`TextNodeStore::measure`] and an image's natural
//! size, and the walk that writes viewport-space rects and commits text
//! boxes. The tree is built afresh for
//! each dirty root: a root is small, and the text engine caches shaping per
//! node, so a clean root costs nothing and a dirty one costs its solve. A
//! scroll panel (M57) hides its vertical overflow, so it can be shorter than
//! its content, and the walk sets its largest offset.

use std::collections::HashMap;

use glam::Vec2;
use taffy::prelude::*;
use taffy::style_helpers::{auto, length, percent};
use taffy::{AvailableSpace, Display, FlexDirection, NodeId, Overflow, Position, TaffyTree};

use crate::text::{MeasureWidth, TextNodeStore};

use super::super::geometry::Rect;
use super::super::id::WidgetId;
use super::super::node::{Visibility, WidgetKind};
use super::super::style::{
    Align, Anchor, Direction, Edges, Justify, LayoutStyle, Placement, Sizing,
};
use super::super::tree::UiTree;

/// Counts from one root's solve.
#[derive(Debug, Default)]
pub(crate) struct RootLayout {
    pub(crate) nodes: u32,
    pub(crate) measure_calls: u32,
    pub(crate) commits: u32,
}

/// Which way an anchor sits on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisAnchor {
    Start,
    Center,
    End,
}

/// The horizontal and vertical placement of `anchor`.
fn axes(anchor: Anchor) -> (AxisAnchor, AxisAnchor) {
    use AxisAnchor::{Center, End, Start};
    match anchor {
        Anchor::TopStart => (Start, Start),
        Anchor::TopCenter => (Center, Start),
        Anchor::TopEnd => (End, Start),
        Anchor::CenterStart => (Start, Center),
        Anchor::Center => (Center, Center),
        Anchor::CenterEnd => (End, Center),
        Anchor::BottomStart => (Start, End),
        Anchor::BottomCenter => (Center, End),
        Anchor::BottomEnd => (End, End),
    }
}

fn direction(direction: Direction) -> FlexDirection {
    match direction {
        Direction::Row => FlexDirection::Row,
        Direction::Column => FlexDirection::Column,
    }
}

fn align(align: Align) -> AlignItems {
    match align {
        Align::Start => AlignItems::START,
        Align::Center => AlignItems::CENTER,
        Align::End => AlignItems::END,
        Align::Stretch => AlignItems::STRETCH,
    }
}

fn justify(justify: Justify) -> JustifyContent {
    match justify {
        Justify::Start => JustifyContent::START,
        Justify::Center => JustifyContent::CENTER,
        Justify::End => JustifyContent::END,
        Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        Justify::SpaceAround => JustifyContent::SPACE_AROUND,
        Justify::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    }
}

fn edges<T: FromLength>(edges: Edges) -> taffy::geometry::Rect<T> {
    taffy::geometry::Rect {
        left: length(edges.start),
        right: length(edges.end),
        top: length(edges.top),
        bottom: length(edges.bottom),
    }
}

fn limit(value: Option<f32>) -> LengthPercentageAuto {
    value.map_or_else(auto, length)
}

/// A fixed size, or `auto`, for an absolute item: `Fill` is the parent's
/// whole axis.
fn absolute_size(sizing: Sizing) -> Dimension {
    match sizing {
        Sizing::Auto => auto(),
        Sizing::Px(v) => length(v),
        Sizing::Percent(p) => percent(p / 100.0),
        Sizing::Fill => percent(1.0),
    }
}

/// Converts `style` for a widget whose parent flows along `parent_dir`
/// (`D-124`'s table). Main axis: `Auto` is content-sized and never shrinks,
/// `Fill` takes the remaining space, `Px` and `Percent` are fixed with shrink
/// 0. Cross axis: `Auto` follows the parent's `align_items`, `Fill`
/// stretches, `Px` and `Percent` are fixed. `Anchored` is an absolute item
/// with insets on its `Start` and `End` edges and inset 0 on a centred axis,
/// which [`place`] shifts after the solve.
pub(crate) fn to_taffy(style: &LayoutStyle, parent_dir: Direction) -> Style {
    let (main, cross) = match parent_dir {
        Direction::Row => (style.width, style.height),
        Direction::Column => (style.height, style.width),
    };
    let anchored = matches!(style.placement, Placement::Anchored { .. });
    let (main_size, flex_basis, flex_grow, flex_shrink) = if anchored {
        (absolute_size(main), auto(), 0.0, 0.0)
    } else {
        match main {
            Sizing::Auto => (auto(), auto(), 0.0, 0.0),
            Sizing::Fill => (auto(), length(0.0), 1.0, 1.0),
            Sizing::Px(v) => (length(v), auto(), 0.0, 0.0),
            Sizing::Percent(p) => (percent(p / 100.0), auto(), 0.0, 0.0),
        }
    };
    let (cross_size, stretch) = if anchored {
        (absolute_size(cross), None)
    } else {
        match cross {
            Sizing::Auto => (auto(), None),
            Sizing::Fill => (auto(), Some(AlignSelf::STRETCH)),
            Sizing::Px(v) => (length(v), None),
            Sizing::Percent(p) => (percent(p / 100.0), None),
        }
    };
    let size = match parent_dir {
        Direction::Row => Size {
            width: main_size,
            height: cross_size,
        },
        Direction::Column => Size {
            width: cross_size,
            height: main_size,
        },
    };
    let mut taffy = Style {
        display: Display::Flex,
        flex_direction: direction(style.direction),
        size,
        min_size: Size {
            width: limit(style.min_width),
            height: limit(style.min_height),
        },
        max_size: Size {
            width: limit(style.max_width),
            height: limit(style.max_height),
        },
        flex_basis,
        flex_grow,
        flex_shrink,
        align_self: stretch.or_else(|| style.align_self.map(align)),
        align_items: Some(align(style.align_items)),
        justify_content: Some(justify(style.justify_content)),
        gap: length(style.gap),
        padding: edges(style.padding),
        margin: edges(style.margin),
        ..Style::default()
    };
    if let Placement::Anchored { anchor, offset } = style.placement {
        taffy.position = Position::Absolute;
        let (x, y) = axes(anchor);
        taffy.inset = taffy::geometry::Rect {
            left: match x {
                AxisAnchor::Start => length(offset.x),
                AxisAnchor::Center => length(0.0),
                AxisAnchor::End => auto(),
            },
            right: match x {
                AxisAnchor::End => length(offset.x),
                _ => auto(),
            },
            top: match y {
                AxisAnchor::Start => length(offset.y),
                AxisAnchor::Center => length(0.0),
                AxisAnchor::End => auto(),
            },
            bottom: match y {
                AxisAnchor::End => length(offset.y),
                _ => auto(),
            },
        };
    }
    taffy
}

/// The synthetic parent every root sits in: a viewport-sized column with the
/// default `LayoutStyle`.
fn viewport_style(viewport: Vec2) -> Style {
    let mut style = to_taffy(&LayoutStyle::default(), Direction::Column);
    style.size = Size {
        width: length(viewport.x),
        height: length(viewport.y),
    };
    style
}

/// A scroll panel's additions (M57): its vertical overflow hidden, which
/// lets it be shorter than its content (Taffy's automatic minimum is then
/// 0), and shrink 1 when its main size is `Auto`, so it takes its content's
/// height up to what its parent's limits leave.
fn scroll_style(style: &mut Style, layout: &LayoutStyle, parent_dir: Direction) {
    style.overflow = taffy::Point {
        x: Overflow::Visible,
        y: Overflow::Hidden,
    };
    let main = match parent_dir {
        Direction::Row => layout.width,
        Direction::Column => layout.height,
    };
    if main == Sizing::Auto {
        style.flex_shrink = 1.0;
    }
}

/// Builds `id`'s subtree into `taffy`, skipping collapsed subtrees. Labels,
/// images, toggles, sliders and meshes are leaves carrying their widget as
/// context.
fn build(
    tree: &UiTree,
    taffy: &mut TaffyTree<WidgetId>,
    id: WidgetId,
    parent_dir: Direction,
    map: &mut HashMap<WidgetId, NodeId>,
) -> Option<NodeId> {
    let node = tree.node(id)?;
    if node.visibility == Visibility::Collapsed {
        return None;
    }
    let mut style = to_taffy(&node.layout_style, parent_dir);
    if node.scroll.is_some() {
        scroll_style(&mut style, &node.layout_style, parent_dir);
    }
    let taffy_node = if node.kind.is_leaf() {
        taffy.new_leaf_with_context(style, id)
    } else {
        let children: Vec<NodeId> = node
            .children
            .iter()
            .filter_map(|child| build(tree, taffy, *child, node.layout_style.direction, map))
            .collect();
        taffy.new_with_children(style, &children)
    }
    .expect("taffy tree building cannot fail");
    map.insert(id, taffy_node);
    Some(taffy_node)
}

fn available(space: AvailableSpace) -> MeasureWidth {
    match space {
        AvailableSpace::Definite(width) => MeasureWidth::Definite(width),
        AvailableSpace::MinContent => MeasureWidth::MinContent,
        AvailableSpace::MaxContent => MeasureWidth::MaxContent,
    }
}

/// Writes `id`'s viewport-space rect from its solved layout under a parent at
/// `parent_pos` of `parent_size`, shifting a centred anchor by half the free
/// space plus its offset, then its children; collects the text leaves with
/// their content boxes, the box less Taffy's border and padding (M56). A
/// scroll panel then takes its largest offset (M57): how far its lowest
/// child's box, margin included, and its bottom padding and border reach
/// past its box, at least 0.
fn place(
    tree: &mut UiTree,
    taffy: &TaffyTree<WidgetId>,
    map: &HashMap<WidgetId, NodeId>,
    id: WidgetId,
    parent_pos: Vec2,
    parent_size: Vec2,
    text_leaves: &mut Vec<(WidgetId, Rect)>,
) {
    let Some(taffy_node) = map.get(&id) else {
        return;
    };
    let layout = taffy
        .layout(*taffy_node)
        .expect("a built node has a layout");
    let size = Vec2::new(layout.size.width, layout.size.height);
    let mut pos = parent_pos + Vec2::new(layout.location.x, layout.location.y);
    let Some(node) = tree.node(id) else {
        return;
    };
    if let Placement::Anchored { anchor, offset } = node.layout_style.placement {
        let (x, y) = axes(anchor);
        if x == AxisAnchor::Center {
            pos.x += (parent_size.x - size.x) / 2.0 + offset.x;
        }
        if y == AxisAnchor::Center {
            pos.y += (parent_size.y - size.y) / 2.0 + offset.y;
        }
    }
    let children = node.children.clone();
    let is_label = node.kind == WidgetKind::Label;
    let scrolls = node.scroll.is_some();
    let content = is_label.then(|| {
        let (padding, border) = (layout.padding, layout.border);
        let start = Vec2::new(padding.left + border.left, padding.top + border.top);
        let end = Vec2::new(padding.right + border.right, padding.bottom + border.bottom);
        Rect::from_pos_size(pos + start, (size - start - end).max(Vec2::ZERO))
    });
    if let Some(node) = tree.node_mut(id) {
        node.rect = Some(Rect::from_pos_size(pos, size));
        node.content = content;
    }
    if let Some(content) = content {
        text_leaves.push((id, content));
    }
    for child in &children {
        place(tree, taffy, map, *child, pos, size, text_leaves);
    }
    if scrolls {
        let insets = (
            layout.padding.top + layout.border.top,
            layout.padding.bottom + layout.border.bottom,
        );
        let furthest = children
            .iter()
            .filter_map(|child| {
                let rect = tree.node(*child)?.rect?;
                let margin = map
                    .get(child)
                    .and_then(|node| taffy.layout(*node).ok())
                    .map_or(0.0, |layout| layout.margin.bottom);
                Some(rect.max.y + margin)
            })
            .reduce(f32::max);
        let max = furthest.map_or(0.0, |furthest| {
            (furthest + insets.1 - (pos.y + size.y)).max(0.0)
        });
        if let Some(state) = tree.node_mut(id).and_then(|node| node.scroll.as_mut()) {
            state.measured(max, insets);
        }
    }
}

/// Clears the rects of `id`'s subtree, so a collapsed node has none.
fn clear_rects(tree: &mut UiTree, id: WidgetId) {
    let Some(node) = tree.node_mut(id) else {
        return;
    };
    node.rect = None;
    node.content = None;
    let children = node.children.clone();
    for child in children {
        clear_rects(tree, child);
    }
}

/// Lays `root` out in a `viewport`-sized parent, its controls' line height
/// times `text_scale` (its layer's, M58), writes the rects and commits
/// every text leaf once.
pub(crate) fn lay_out_root(
    tree: &mut UiTree,
    root: WidgetId,
    viewport: Vec2,
    text_scale: f32,
    store: &mut dyn TextNodeStore,
) -> RootLayout {
    let mut result = RootLayout::default();
    clear_rects(tree, root);

    let mut taffy: TaffyTree<WidgetId> = TaffyTree::new();
    taffy.disable_rounding();
    let mut map = HashMap::new();
    let Some(root_node) = build(tree, &mut taffy, root, Direction::Column, &mut map) else {
        return result;
    };
    let viewport_node = taffy
        .new_with_children(viewport_style(viewport), &[root_node])
        .expect("taffy tree building cannot fail");
    result.nodes = map.len() as u32;

    let mut measure_calls = 0_u32;
    taffy
        .compute_layout_with_measure(
            viewport_node,
            Size {
                width: AvailableSpace::Definite(viewport.x),
                height: AvailableSpace::Definite(viewport.y),
            },
            |inputs, _node, context, style| {
                taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, space| {
                        let Some(node) = context.and_then(|id| tree.node(*id)) else {
                            return Size::ZERO;
                        };
                        if let (Some(width), Some(height)) = (known.width, known.height) {
                            return Size { width, height };
                        }
                        if let Some(image) = node.image {
                            return Size {
                                width: known.width.unwrap_or(image.natural_size.x),
                                height: known.height.unwrap_or(image.natural_size.y),
                            };
                        }
                        if let Some(control) = node.control {
                            // The text scale grows a control with its line
                            // height (M56), on the game layers (M58).
                            let line = node
                                .resolved
                                .as_ref()
                                .map_or(0.0, |resolved| resolved.text.line_height)
                                * text_scale;
                            let (width, height) = control.natural_size(line);
                            return Size {
                                width: known.width.unwrap_or(width),
                                height: known.height.unwrap_or(height),
                            };
                        }
                        let Some(text_node) = node.text_id else {
                            return Size::ZERO;
                        };
                        measure_calls += 1;
                        // A known width is the border box's; the available
                        // width is then the content box's, where the text
                        // commits (M56).
                        let width = known.width.map(|width| match space.width {
                            AvailableSpace::Definite(content) => content,
                            _ => width,
                        });
                        let metrics = store.measure(text_node, width, available(space.width));
                        Size {
                            width: known.width.unwrap_or(metrics.size.x),
                            height: known.height.unwrap_or(metrics.size.y),
                        }
                    },
                )
            },
        )
        .expect("layout of a built tree cannot fail");
    result.measure_calls = measure_calls;

    let mut text_leaves = Vec::new();
    place(
        tree,
        &taffy,
        &map,
        root,
        Vec2::ZERO,
        viewport,
        &mut text_leaves,
    );
    for (id, content) in text_leaves {
        let Some(text_node) = tree.node(id).and_then(|node| node.text_id) else {
            continue;
        };
        store.commit_layout(text_node, content.size());
        result.commits += 1;
    }
    result
}
