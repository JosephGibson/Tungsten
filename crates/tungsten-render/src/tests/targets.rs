use super::*;

#[test]
fn non_srgb_twin_maps_rgba8_unorm_srgb() {
    assert_eq!(
        non_srgb_twin(TextureFormat::Rgba8UnormSrgb),
        Some(TextureFormat::Rgba8Unorm)
    );
}

#[test]
fn non_srgb_twin_maps_bgra8_unorm_srgb() {
    assert_eq!(
        non_srgb_twin(TextureFormat::Bgra8UnormSrgb),
        Some(TextureFormat::Bgra8Unorm)
    );
}

#[test]
fn non_srgb_twin_returns_none_for_linear_input() {
    assert_eq!(non_srgb_twin(TextureFormat::Rgba8Unorm), None);
    assert_eq!(non_srgb_twin(TextureFormat::Bgra8Unorm), None);
}

#[test]
fn target_cache_rebuilds_only_when_its_key_moves() {
    // Keyed like the renderer's caches: pool generation and source target.
    let mut cache: TargetCache<(u64, TargetId), u32> = TargetCache::default();
    let mut builds = 0;
    let mut get = |cache: &mut TargetCache<(u64, TargetId), u32>, key| {
        *cache.get_or_build(key, || {
            builds += 1;
            builds
        })
    };
    assert_eq!(get(&mut cache, (0, TargetId::SceneColor)), 1);
    assert_eq!(get(&mut cache, (0, TargetId::SceneColor)), 1);
    // A resize or a `post_aa` switch rebuilds the targets: new generation.
    assert_eq!(get(&mut cache, (1, TargetId::SceneColor)), 2);
    assert_eq!(get(&mut cache, (1, TargetId::SceneColor)), 2);
    // Another source target in the same generation.
    assert_eq!(get(&mut cache, (1, TargetId::PostPing)), 3);
    cache.clear();
    assert_eq!(get(&mut cache, (1, TargetId::PostPing)), 4);
}

#[test]
fn bloom_mip_count_clamps_to_viewport_size() {
    // 1080p tall enough for the requested 6 mips at half-res start.
    assert_eq!(bloom_mip_count_for_size(1920, 1080, 6), 6);
    // 64x64 viewport: floor(log2(64)) = 6, minus 1 = 5 mip ceiling.
    assert_eq!(bloom_mip_count_for_size(64, 64, 6), 5);
    // Tiny viewport: floor must not underflow.
    assert_eq!(bloom_mip_count_for_size(2, 2, 6), 1);
    assert_eq!(bloom_mip_count_for_size(1, 1, 6), 1);
    // max_mips = 0 still yields at least 1.
    assert_eq!(bloom_mip_count_for_size(1024, 1024, 0), 1);
    // Larger pyramids respect the viewport ceiling.
    assert_eq!(bloom_mip_count_for_size(1024, 1024, 8), 8);
}
