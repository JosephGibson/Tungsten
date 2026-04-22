use super::*;

#[test]
fn default_is_empty() {
    let dd = DebugDraw::new();
    assert!(dd.is_empty());
    assert_eq!(dd.len(), 0);
}

#[test]
fn draw_aabb_pushes_one_command() {
    let mut dd = DebugDraw::new();
    dd.draw_aabb(Vec2::ZERO, Vec2::splat(1.0), [1.0; 4], 1.0);
    assert_eq!(dd.len(), 1);
    match dd.commands()[0].shape {
        DebugShape::Aabb { min, max } => {
            assert_eq!(min, Vec2::ZERO);
            assert_eq!(max, Vec2::splat(1.0));
        }
        _ => panic!("wrong shape"),
    }
}

#[test]
fn draw_circle_uses_default_segments() {
    let mut dd = DebugDraw::new();
    dd.draw_circle(Vec2::ZERO, 5.0, [0.0, 1.0, 0.0, 1.0], 1.0);
    assert_eq!(dd.len(), 1);
    match dd.commands()[0].shape {
        DebugShape::Circle {
            center,
            radius,
            segments,
        } => {
            assert_eq!(center, Vec2::ZERO);
            assert_eq!(radius, 5.0);
            assert_eq!(segments, DEFAULT_CIRCLE_SEGMENTS);
        }
        _ => panic!("wrong shape"),
    }
}

#[test]
fn draw_line_pushes_one_command() {
    let mut dd = DebugDraw::new();
    dd.draw_line(Vec2::ZERO, Vec2::splat(2.0), [1.0; 4], 2.0);
    assert_eq!(dd.len(), 1);
}

#[test]
fn clear_empties_queue() {
    let mut dd = DebugDraw::new();
    dd.draw_line(Vec2::ZERO, Vec2::ONE, [1.0; 4], 1.0);
    dd.draw_line(Vec2::ZERO, Vec2::ONE, [1.0; 4], 1.0);
    assert_eq!(dd.len(), 2);
    dd.clear();
    assert!(dd.is_empty());
}

#[test]
fn drain_empties_and_yields_commands() {
    let mut dd = DebugDraw::new();
    dd.draw_aabb(Vec2::ZERO, Vec2::ONE, [1.0; 4], 1.0);
    dd.draw_circle(Vec2::ZERO, 1.0, [1.0; 4], 1.0);
    let collected: Vec<DebugCommand> = dd.drain().collect();
    assert_eq!(collected.len(), 2);
    assert!(dd.is_empty());
}
