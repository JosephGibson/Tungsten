use tungsten_core::assets::{
    AnimationData, AnimationRegistry, ResolvedManifest, SoundData, SoundRegistry, TextureHandle,
};
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

/// Load all font assets from a resolved manifest: read TTF bytes and
/// register them in the renderer's text pipeline.
pub fn load_fonts(manifest: &ResolvedManifest, renderer: &mut Renderer) -> anyhow::Result<()> {
    for (id, font_entry) in &manifest.fonts {
        let data = std::fs::read(&font_entry.path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read font '{}' at '{}': {}",
                id,
                font_entry.path.display(),
                e
            )
        })?;
        log::info!(
            "Loaded font '{}' ({} bytes) from '{}'",
            id,
            data.len(),
            font_entry.path.display(),
        );
        renderer.load_font(id, data);
    }
    Ok(())
}

/// Load all sound assets from a resolved manifest: decode audio files and
/// register them in the `SoundRegistry` resource.
pub fn load_sounds(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut sound_registry = SoundRegistry::new();

    for (id, sound_entry) in &manifest.sounds {
        let data = SoundData::decode(&sound_entry.path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to decode sound '{}' at '{}': {}",
                id,
                sound_entry.path.display(),
                e
            )
        })?;
        log::info!(
            "Loaded sound '{}' ({} samples, {}Hz, {} ch) from '{}'",
            id,
            data.samples.len(),
            data.sample_rate,
            data.channels,
            sound_entry.path.display(),
        );
        sound_registry.register(id.clone(), data, sound_entry.volume, sound_entry.looping);
    }

    world.insert_resource(sound_registry);
    Ok(())
}

/// Load all assets (sprites + animations + fonts + sounds) from a manifest.
/// After loading, validates that every sprite ID referenced from animation
/// frames exists in the sprite registry (D-009).
pub fn load_all(
    manifest: &ResolvedManifest,
    world: &mut World,
    renderer: &mut Renderer,
) -> anyhow::Result<()> {
    load_sprites(manifest, world, renderer)?;
    load_animations(manifest, world)?;
    load_fonts(manifest, renderer)?;
    load_sounds(manifest, world)?;

    let registry = world
        .get_resource::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    let anim_registry = world
        .get_resource::<AnimationRegistry>()
        .expect("AnimationRegistry resource missing");

    for (anim_id, anim_data) in anim_registry.iter() {
        for (i, frame) in anim_data.frames.iter().enumerate() {
            if registry.get_sprite(&frame.sprite).is_none() {
                return Err(anyhow::anyhow!(
                    "Animation '{}' frame {} references unknown sprite ID '{}'",
                    anim_id,
                    i,
                    frame.sprite,
                ));
            }
        }
    }

    Ok(())
}
