use super::*;

#[test]
fn transform_default_is_identity() {
    let t = Transform::default();
    assert_eq!(t.position, Vec2::ZERO);
    assert_eq!(t.rotation, 0.0);
    assert_eq!(t.scale, Vec2::ONE);
}

#[test]
fn transform_from_position_sets_position_only() {
    let t = Transform::from_position(Vec2::new(7.0, -2.0));
    assert_eq!(t.position, Vec2::new(7.0, -2.0));
    assert_eq!(t.rotation, 0.0);
    assert_eq!(t.scale, Vec2::ONE);
}

#[test]
fn visibility_default_is_visible() {
    assert!(Visibility::default().visible);
}

#[test]
fn sprite_new_defaults_color_and_z_order() {
    let s = Sprite::new("player");
    assert_eq!(s.asset_id, "player");
    assert_eq!(s.color, [255; 4]);
    assert_eq!(s.z_order, 0);
}

#[test]
fn tag_new_stores_name() {
    let t = Tag::new("hero");
    assert_eq!(t.name, "hero");
}

#[test]
fn sync_position_to_transform_copies_position() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position(Vec2::new(3.0, 4.0)));
    world.insert(
        e,
        Transform {
            position: Vec2::ZERO,
            rotation: 1.5,
            scale: Vec2::splat(2.0),
        },
    );

    sync_position_to_transform(&mut world);

    let t = world.get::<Transform>(e).unwrap();
    assert_eq!(t.position, Vec2::new(3.0, 4.0));
    assert_eq!(t.rotation, 1.5);
    assert_eq!(t.scale, Vec2::splat(2.0));
}

#[test]
fn sync_position_to_transform_skips_entities_missing_either() {
    let mut world = World::new();
    let only_position = world.spawn();
    world.insert(only_position, Position(Vec2::new(9.0, 9.0)));

    let only_transform = world.spawn();
    world.insert(only_transform, Transform::default());

    sync_position_to_transform(&mut world);

    assert!(world.get::<Transform>(only_position).is_none());
    assert_eq!(
        world.get::<Transform>(only_transform).unwrap().position,
        Vec2::ZERO
    );
}

#[test]
fn light_point_constructor_intensity_one() {
    let l = Light::point(Vec3::new(1.0, 0.5, 0.25), 4.0);
    assert_eq!(l.color, Vec3::new(1.0, 0.5, 0.25));
    assert_eq!(l.intensity, 1.0);
    match l.kind {
        LightKind::Point { radius, falloff } => {
            assert_eq!(radius, 4.0);
            assert_eq!(falloff, 1.0);
        }
        LightKind::Directional { .. } => panic!("expected Point"),
    }
}

#[test]
fn light_directional_constructor_angle() {
    let l = Light::directional(Vec3::ONE, std::f32::consts::FRAC_PI_4);
    assert_eq!(l.intensity, 1.0);
    match l.kind {
        LightKind::Directional { angle } => assert_eq!(angle, std::f32::consts::FRAC_PI_4),
        LightKind::Point { .. } => panic!("expected Directional"),
    }
}

#[test]
fn sync_position_to_transform_does_not_touch_position() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Position(Vec2::new(1.0, 2.0)));
    world.insert(e, Transform::default());

    sync_position_to_transform(&mut world);

    world.get_mut::<Transform>(e).unwrap().position = Vec2::splat(42.0);
    assert_eq!(world.get::<Position>(e).unwrap().0, Vec2::new(1.0, 2.0));
}
