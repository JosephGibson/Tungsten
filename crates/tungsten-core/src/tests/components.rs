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
    let s = Sprite::new(SpriteAssetId::new(7));
    assert_eq!(s.asset_id, SpriteAssetId::new(7));
    assert_eq!(s.color, [255; 4]);
    assert_eq!(s.z_order, 0);
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
        LightKind::Point { radius } => assert_eq!(radius, 4.0),
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

#[test]
fn parallax_identity_factor_is_world_locked() {
    let authored = Vec2::new(120.0, -45.0);
    let camera = Vec2::new(900.0, 300.0);
    assert_eq!(
        parallax_world_position(authored, Vec2::ONE, camera),
        authored
    );
}

#[test]
fn parallax_zero_factor_is_screen_locked() {
    let authored = Vec2::new(10.0, 20.0);
    let camera = Vec2::new(640.0, -360.0);
    // Screen-locked: the layer rides the camera, so its on-screen offset from
    // the camera origin stays `authored` at every camera position.
    let remapped = parallax_world_position(authored, Vec2::ZERO, camera);
    assert_eq!(remapped, authored + camera);
    assert_eq!(remapped - camera, authored);
}

#[test]
fn parallax_applies_factors_per_axis() {
    let remapped = parallax_world_position(
        Vec2::new(100.0, 200.0),
        Vec2::new(0.25, 1.0),
        Vec2::new(400.0, 800.0),
    );
    assert_eq!(remapped, Vec2::new(100.0 + 400.0 * 0.75, 200.0));
}

#[test]
fn parallax_handles_negative_camera_positions() {
    let remapped = parallax_world_position(
        Vec2::new(0.0, 0.0),
        Vec2::splat(0.5),
        Vec2::new(-200.0, -100.0),
    );
    assert_eq!(remapped, Vec2::new(-100.0, -50.0));
}

#[test]
fn parallax_layer_uniform_splats_both_axes() {
    assert_eq!(
        ParallaxLayer::uniform(0.35).scroll_factor,
        Vec2::splat(0.35)
    );
    assert_eq!(
        ParallaxLayer::new(Vec2::new(0.2, 0.8)).scroll_factor,
        Vec2::new(0.2, 0.8)
    );
}

fn squash(duration: f32) -> SpriteSquashStretch {
    SpriteSquashStretch {
        on: SquashTrigger::OnLand,
        amount: Vec2::new(1.25, 0.75),
        duration,
        easing: Easing::Linear,
    }
}

#[test]
fn squash_envelope_endpoints_return_base_scale() {
    let s = squash(0.2);
    let base = Vec2::new(2.0, 3.0);
    assert_eq!(s.scale_at(base, 0.0), base);
    assert_eq!(s.scale_at(base, 0.2), base);
    assert_eq!(s.scale_at(base, 5.0), base);
}

#[test]
fn squash_envelope_midpoint_hits_amount() {
    let s = squash(0.2);
    let got = s.scale_at(Vec2::ONE, 0.1);
    let delta = (got - s.amount).abs();
    assert!(delta.x <= 1e-6 && delta.y <= 1e-6, "got {got:?}");
}

#[test]
fn squash_envelope_scales_relative_to_base() {
    let s = squash(0.2);
    let base = Vec2::new(4.0, 8.0);
    let got = s.scale_at(base, 0.1);
    let expected = base * s.amount;
    let delta = (got - expected).abs();
    assert!(delta.x <= 1e-5 && delta.y <= 1e-5, "got {got:?}");
}

#[test]
fn squash_envelope_zero_duration_is_inert() {
    let base = Vec2::new(1.5, 1.5);
    assert_eq!(squash(0.0).scale_at(base, 0.0), base);
    assert_eq!(squash(0.0).scale_at(base, 0.5), base);
    assert_eq!(squash(-1.0).scale_at(base, 0.5), base);
}

// Redo 7.10p: named sprite points.

#[test]
fn sprite_point_follows_flip_scale_and_turn() {
    use crate::assets::{AssetRegistry, FilterMode, TextureHandle, UvRect};
    use std::path::PathBuf;

    let mut world = World::new();
    let mut assets = AssetRegistry::new();
    for id in ["keg", "plain"] {
        assets.register_sprite(
            id.into(),
            FilterMode::Nearest,
            16,
            32,
            PathBuf::from(format!("{id}.png")),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
        assets.set_sprite_points(id, [("fuse".to_string(), [4.0, 0.0])].into());
    }
    assets.set_sprite_frame("keg", None, Some([8.0, 32.0]));
    let keg = assets.sprite_id("keg").unwrap();
    let plain = assets.sprite_id("plain").unwrap();
    world.insert_resource(assets);
    let at = Vec2::new(100.0, 50.0);
    let spawn = |world: &mut World, sprite, rotation, scale, flip| {
        world.spawn_with((
            Transform {
                position: at,
                rotation,
                scale: Vec2::splat(scale),
            },
            Sprite::new(sprite),
            SpriteFlip(flip),
        ))
    };
    let close = |got: Option<Vec2>, want: Vec2| got.is_some_and(|got| got.distance(want) < 1e-4);
    // Anchored at its bottom centre and doubled: the fuse 8 px right of the
    // anchor's column, 64 px up.
    let upright = spawn(&mut world, keg, 0.0, 2.0, false);
    assert!(close(
        sprite_point(&world, upright, "fuse"),
        Vec2::new(92.0, -14.0)
    ));
    // Flipped, it mirrors about the anchor.
    let flipped = spawn(&mut world, keg, 0.0, 2.0, true);
    assert!(close(
        sprite_point(&world, flipped, "fuse"),
        Vec2::new(108.0, -14.0)
    ));
    // Turned a quarter, about the anchor on the entity.
    let turned = spawn(&mut world, keg, std::f32::consts::FRAC_PI_2, 2.0, false);
    assert!(close(
        sprite_point(&world, turned, "fuse"),
        Vec2::new(164.0, 42.0)
    ));
    // Without an anchor it turns about the quad's centre.
    let centred = spawn(&mut world, plain, std::f32::consts::FRAC_PI_2, 1.0, false);
    assert!(close(
        sprite_point(&world, centred, "fuse"),
        Vec2::new(124.0, 62.0)
    ));
    assert_eq!(
        sprite_point(&world, upright, "spout"),
        None,
        "no such point"
    );
    let bare = world.spawn_with((Sprite::new(keg),));
    assert_eq!(sprite_point(&world, bare, "fuse"), None, "no Transform");
    let unregistered = world.spawn_with((
        Transform::from_position(at),
        Sprite::new(SpriteAssetId::new(9)),
    ));
    assert_eq!(sprite_point(&world, unregistered, "fuse"), None);
}
