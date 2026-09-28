//! Deterministic render attribution workload: mixed batches, materials, lights,
//! bloom + vignette, SMAA and text. Uses the root manifest's existing assets.

use glam::{Vec2, Vec3};
use tungsten::core::post::{BloomParams, PostPass, PostStack, VignetteParams};
use tungsten::core::tween::UniformOverrideBlock;
use tungsten::core::{
    AmbientLight, DeltaTime, Light, MaterialRegistry, Sprite, Transform, Visibility, World,
};
use tungsten::render::TextSection;
use tungsten::{App, extract_sprites_default};

pub(crate) const DEFAULT_RENDER_FEATURES_COUNT: usize = 4_000;
const SPRITES: [&str; 4] = ["red_square", "blue_square", "green_circle", "walk_0"];

#[derive(Clone, Copy)]
struct RenderFeatureSprite;

pub(crate) fn configure_render_features_scene(app: &mut App, count: usize) {
    app.set_manifest_roots(vec!["assets/manifest.json".into()]);
    app.on_startup(move |world, _| {
        seed_render_features(world, count);
        // Confirm the real atlas/material loader exercises all intended paths.
        let batches = extract_sprites_default(world);
        if count >= 4 {
            assert!(
                batches.iter().any(|b| b.lit),
                "render-features needs a lit batch"
            );
            assert!(batches.iter().any(|b| b.material_id.is_some()));
            assert!(batches.iter().any(|b| !b.lit && b.material_id.is_none()));
        }
        log::info!(
            "render-features: {count} sprites, {} batches, {} lit, {} material",
            batches.len(),
            batches.iter().filter(|b| b.lit).count(),
            batches.iter().filter(|b| b.material_id.is_some()).count()
        );
    });
    app.add_system_named("animate_render_features", animate_render_features);
    app.set_extract_sprites(extract_sprites_default);
    app.set_extract_text(|_| {
        vec![TextSection {
            content: "Render Features — materials / lights / bloom / SMAA".into(),
            font_id: "sans_bold".into(),
            font_size: 26.0,
            line_height: 30.0,
            color: [255; 4],
            position: [16.0, 14.0],
            bounds: None,
        }]
    });
}

fn seed_render_features(world: &mut World, count: usize) {
    let material = world
        .get_resource::<MaterialRegistry>()
        .and_then(|r| r.get("damage_flash"))
        .expect("root manifest must provide damage_flash");
    world.insert_resource(AmbientLight(Vec3::splat(0.25)));
    world.insert_resource(PostStack(vec![
        PostPass::Bloom(BloomParams {
            threshold: 0.55,
            knee: 0.3,
            intensity: 0.8,
            radius: 1.0,
        }),
        PostPass::Vignette(VignetteParams::default()),
    ]));
    let cols = ((count as f32 * 16.0 / 9.0).sqrt().ceil() as usize).max(1);
    let rows = count.div_ceil(cols).max(1);
    for i in 0..count {
        let e = world.spawn();
        let kind = i % SPRITES.len();
        let mut sprite = Sprite::new(SPRITES[kind]);
        sprite.z_order = (i % 3) as i32;
        if kind == 1 {
            sprite.material_id = Some(material);
            let mut f32s = [0.0; 4];
            f32s[0] = if (i / 4).is_multiple_of(2) {
                0.25
            } else {
                0.65
            };
            world.insert(
                e,
                UniformOverrideBlock::from_slots([[1.0; 4]; 4], f32s, [0; 4]),
            );
        }
        let mut transform = Transform::from_position(Vec2::new(
            32.0 + (i % cols) as f32 * 1820.0 / cols as f32,
            80.0 + (i / cols) as f32 * 920.0 / rows as f32,
        ));
        transform.scale = Vec2::splat(0.65);
        transform.rotation = (i % 17) as f32 * 0.05;
        world.insert(e, transform);
        world.insert(e, sprite);
        world.insert(e, Visibility::default());
        world.insert(e, RenderFeatureSprite);
    }
    for (position, light) in [
        (
            Vec2::new(450.0, 400.0),
            Light::point(Vec3::new(1.0, 0.6, 0.2), 700.0),
        ),
        (
            Vec2::new(1450.0, 700.0),
            Light::point(Vec3::new(0.3, 0.6, 1.0), 700.0),
        ),
        (Vec2::ZERO, Light::directional(Vec3::splat(0.35), -0.8)),
    ] {
        let e = world.spawn();
        world.insert(e, Transform::from_position(position));
        world.insert(e, light);
    }
}

fn animate_render_features(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(1.0 / 60.0, DeltaTime::seconds);
    for (_, transform, _) in world.query2_mut::<Transform, RenderFeatureSprite>() {
        transform.rotation = (transform.rotation + dt * 0.3) % std::f32::consts::TAU;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten::core::assets::UvRect;
    use tungsten::core::{AssetRegistry, FilterMode, MaterialUniformDefaults, TextureHandle};

    #[test]
    fn mixed_scene_exercises_batch_keys_and_render_features() {
        let mut world = World::new();
        let mut materials = MaterialRegistry::new();
        materials.allocate(
            "damage_flash",
            "test-manifest".into(),
            "damage_flash".into(),
            MaterialUniformDefaults::default(),
        );
        world.insert_resource(materials);
        let mut assets = AssetRegistry::new();
        for (i, id) in SPRITES.iter().enumerate() {
            let filter = if i == 2 {
                FilterMode::Linear
            } else {
                FilterMode::Nearest
            };
            assets.register_sprite(
                (*id).into(),
                filter,
                32,
                32,
                (*id).into(),
                TextureHandle(i as u32),
                UvRect::FULL,
                None,
                None,
                (i == 3).then_some(TextureHandle(3)),
            );
        }
        world.insert_resource(assets);
        seed_render_features(&mut world, 96);
        let batches = extract_sprites_default(&world);
        assert_eq!(batches.iter().map(|b| b.instances.len()).sum::<usize>(), 96);
        assert!(batches.len() >= 12);
        assert!(batches.iter().any(|b| b.lit));
        assert!(
            batches
                .iter()
                .any(|b| b.material_id.is_some() && b.uniform_overrides.is_some())
        );
        assert!(batches.iter().any(|b| !b.lit && b.material_id.is_none()));
        assert_eq!(world.query::<Light>().count(), 3);
        assert_eq!(world.get_resource::<PostStack>().unwrap().len(), 2);
        let before = world.query::<RenderFeatureSprite>().next().unwrap().0;
        let rotation = world.get::<Transform>(before).unwrap().rotation;
        animate_render_features(&mut world);
        assert!(world.get::<Transform>(before).unwrap().rotation > rotation);
    }
}
