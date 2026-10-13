/// `ui_route_system` bench (`D-149`): one routing per Criterion iteration,
/// of an empty tree and stream, and of a laid-out menu of 100 buttons with a
/// move and a click.
use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec2;
use std::hint::black_box;
use tungsten_core::ui::{
    Anchor, FixedAdvanceMeasure, LayoutStyle, Sizing, UiLayer, ui_route_system,
};
use tungsten_core::{
    ActionMap, InputState, MouseButton, RoutedInput, ScaleFactor, Time, UiEvent, UiRouter, UiTree,
    World,
};

const VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);

fn world(tree: UiTree) -> World {
    let mut world = World::new();
    world.insert_resource(InputState::new());
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(Time::new());
    world.insert_resource(ScaleFactor::default());
    world.insert_resource(UiRouter::default());
    world.insert_resource(RoutedInput::default());
    world.register_event::<UiEvent>();
    world.insert_resource(tree);
    world
}

fn bench_empty(c: &mut Criterion) {
    let mut tree = UiTree::new();
    tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    let mut world = world(tree);

    c.bench_function("ui_route_empty", |b| {
        b.iter(|| ui_route_system(black_box(&mut world)));
    });
}

fn bench_menu_move_click(c: &mut Criterion) {
    let mut tree = UiTree::new();
    let root = tree.add_root(
        UiLayer::Game,
        LayoutStyle::default().with_size(Sizing::Px(VIEWPORT.x), Sizing::Px(VIEWPORT.y)),
    );
    let mut buttons = Vec::new();
    for row in 0..10 {
        for column in 0..10 {
            let button = tree.button(root, format!("Item {row}-{column}"));
            tree.set_layout_style(
                button,
                LayoutStyle::default()
                    .with_size(Sizing::Px(120.0), Sizing::Px(60.0))
                    .anchored(
                        Anchor::TopStart,
                        Vec2::new(column as f32 * 128.0, row as f32 * 72.0),
                    ),
            );
            buttons.push(button);
        }
    }
    tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    let rect = tree.rect(buttons[55]).expect("laid out");
    let target = (rect.min + rect.max) * 0.5;
    let mut world = world(tree);

    c.bench_function("ui_route_menu_100_move_click", |b| {
        b.iter(|| {
            world.flush_events();
            let input = world.resource_mut::<InputState>();
            input.begin_frame();
            input.update_cursor_position(target.x, target.y);
            input.mouse_down(MouseButton::Left);
            input.mouse_up(MouseButton::Left);
            ui_route_system(black_box(&mut world));
        });
    });
}

criterion_group!(benches, bench_empty, bench_menu_move_click);
criterion_main!(benches);
