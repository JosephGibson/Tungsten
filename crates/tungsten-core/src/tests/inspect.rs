use super::*;
use glam::Vec2;

#[test]
fn tag_emits_single_name_row() {
    let t = Tag::new("hero");
    let rows = t.inspect_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "name");
    assert_eq!(rows[0].1, "hero");
}

#[test]
fn transform_emits_three_rows() {
    let t = Transform {
        position: Vec2::new(1.0, 2.0),
        rotation: 0.5,
        scale: Vec2::new(2.0, 3.0),
    };
    let rows = t.inspect_rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].0, "pos");
    assert_eq!(rows[1].0, "rot");
    assert_eq!(rows[2].0, "scale");
}

#[test]
fn visibility_emits_single_row() {
    let v = Visibility { visible: true };
    let rows = v.inspect_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "visible");
    assert_eq!(rows[0].1, "true");
}

#[test]
fn sprite_emits_three_rows() {
    let s = Sprite::new("player");
    let rows = s.inspect_rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].0, "asset");
    assert_eq!(rows[0].1, "player");
    assert_eq!(rows[1].0, "tint");
    assert_eq!(rows[2].0, "z");
}

#[test]
fn position_emits_single_row() {
    let p = Position(Vec2::new(3.0, 4.0));
    let rows = p.inspect_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "pos");
}

#[test]
fn velocity_emits_single_row() {
    let v = Velocity(Vec2::new(3.0, 4.0));
    let rows = v.inspect_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "vel");
}
