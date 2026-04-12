use tungsten_core::assets::{AnimationData, AnimationRegistry, ResolvedManifest, TextureHandle};
use tungsten_core::{AssetRegistry, World};
use tungsten_render::Renderer;

/// Load all sprite assets from a resolved manifest: decode PNGs to CPU bitmaps,
/// register in the asset registry, and upload textures to the GPU.
pub fn load_sprites(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");

    let mut uploads: Vec<(TextureHandle, Vec<u8>, u32, u32)> = Vec::new();

    for (id, sprite) in &manifest.sprites {
        let img = image::open(&sprite.path)
            .map_err(|e| anyhow::anyhow!("Failed to decode '{}': {}", sprite.path.display(), e))?
            .to_rgba8();

        let (width, height) = img.dimensions();
        let handle = registry.register_sprite(id.clone(), sprite.filter, width, height);

        log::info!(
            "Loaded sprite '{}' ({}x{}, {:?}) -> {:?}",
            id,
            width,
            height,
            sprite.filter,
            handle,
        );

        uploads.push((handle, img.into_raw(), width, height));
    }

    for (handle, rgba, width, height) in uploads {
        renderer.upload_texture(handle, &rgba, width, height);
    }

    Ok(())
}

/// Load all animation data from a resolved manifest.
pub fn load_animations(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut anim_registry = AnimationRegistry::new();

    for (id, anim_entry) in &manifest.animations {
        let data = AnimationData::load(&anim_entry.path)?;
        log::info!(
            "Loaded animation '{}' ({} frames, {}ms total, looping={})",
            id,
            data.frames.len(),
            data.total_duration_ms(),
            data.looping,
        );
        anim_registry.insert(id.clone(), data);
    }

    world.insert_resource(anim_registry);
    Ok(())
}

/// Load all assets (sprites + animations) from a manifest.
pub fn load_all(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    load_sprites(manifest, world, renderer)?;
    load_animations(manifest, world)?;
    Ok(())
}
