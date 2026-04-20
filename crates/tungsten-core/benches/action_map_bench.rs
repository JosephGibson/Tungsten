/// M19 ActionMap query benchmark — one query per iteration so Criterion's
/// reported time is directly comparable to the ≤ 1 µs per-call target for
/// keyboard and mouse-source dispatch.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tungsten_core::{ActionMap, InputState, KeyCode, MouseButton};

fn bench_is_pressed_key(c: &mut Criterion) {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.key_down(KeyCode::KeyA);

    c.bench_function("action_map_is_pressed_key", |b| {
        b.iter(|| {
            let hit = map.is_pressed(black_box(&input), black_box("move_left"));
            black_box(hit);
        });
    });
}

fn bench_just_pressed_key(c: &mut Criterion) {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.key_down(KeyCode::Space);

    c.bench_function("action_map_just_pressed_key", |b| {
        b.iter(|| {
            let hit = map.just_pressed(black_box(&input), black_box("jump"));
            black_box(hit);
        });
    });
}

fn bench_is_pressed_mouse_button(c: &mut Criterion) {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.mouse_down(MouseButton::Left);

    c.bench_function("action_map_is_pressed_mouse_button", |b| {
        b.iter(|| {
            let hit = map.is_pressed(black_box(&input), black_box("jump"));
            black_box(hit);
        });
    });
}

fn bench_just_pressed_scroll(c: &mut Criterion) {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.add_scroll_line_delta(0.0, 1.0);

    c.bench_function("action_map_just_pressed_scroll", |b| {
        b.iter(|| {
            let hit = map.just_pressed(black_box(&input), black_box("zoom_in"));
            black_box(hit);
        });
    });
}

fn bench_is_pressed_unknown_action(c: &mut Criterion) {
    let map = ActionMap::default_map();
    let input = InputState::new();

    c.bench_function("action_map_is_pressed_unknown", |b| {
        b.iter(|| {
            let hit = map.is_pressed(black_box(&input), black_box("does_not_exist"));
            black_box(hit);
        });
    });
}

criterion_group!(
    benches,
    bench_is_pressed_key,
    bench_just_pressed_key,
    bench_is_pressed_mouse_button,
    bench_just_pressed_scroll,
    bench_is_pressed_unknown_action,
);
criterion_main!(benches);
