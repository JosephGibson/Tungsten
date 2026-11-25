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
    // Keyed like the renderer's caches: the identity of the pooled texture
    // the value holds a view of.
    let mut cache: TargetCache<u64, u32> = TargetCache::default();
    let mut builds = 0;
    let mut get = |cache: &mut TargetCache<u64, u32>, key| {
        *cache.get_or_build(key, || {
            builds += 1;
            builds
        })
    };
    assert_eq!(cache.get(), None);
    assert_eq!(get(&mut cache, 7), 1);
    assert_eq!(get(&mut cache, 7), 1);
    assert_eq!(cache.get(), Some(&1));
    // Another texture: the frame's shape moved, or a resize made new ones.
    assert_eq!(get(&mut cache, 8), 2);
    assert_eq!(get(&mut cache, 8), 2);
    // A freed texture takes the value over it, and no other texture's.
    cache.clear_if(|key| *key == 7);
    assert_eq!(cache.get(), Some(&2));
    assert_eq!(get(&mut cache, 8), 2);
    cache.clear_if(|key| *key == 8);
    assert_eq!(cache.get(), None);
    assert_eq!(get(&mut cache, 8), 3);
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

#[test]
fn blooms_first_level_follows_the_targets_rows() {
    use tungsten_core::config::BloomBase;

    // `auto`: a half up to 1440 rows, a quarter above (`D-301`).
    let first = |size: (u32, u32), base: BloomBase| {
        bloom_mip_extent_for(size.0, size.1, base.halvings(size.1), 0)
    };
    assert_eq!(first((1280, 720), BloomBase::Auto), (640, 360));
    assert_eq!(first((1920, 1080), BloomBase::Auto), (960, 540));
    assert_eq!(first((2560, 1440), BloomBase::Auto), (1280, 720));
    assert_eq!(first((2560, 1441), BloomBase::Auto), (640, 360));
    assert_eq!(first((3840, 2160), BloomBase::Auto), (960, 540));
    // `half` is the pyramid of before the key at any size, `quarter` its
    // second level at any size.
    for size in [(1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)] {
        assert_eq!(first(size, BloomBase::Half), (size.0 / 2, size.1 / 2));
        assert_eq!(first(size, BloomBase::Quarter), (size.0 / 4, size.1 / 4));
    }

    // At 2160 rows the default six levels are 1080 rows' own.
    let levels = |size: (u32, u32), base: BloomBase| -> Vec<(u32, u32)> {
        let halvings = base.halvings(size.1);
        let count = bloom_mip_count_for_halvings(size.0, size.1, 6, halvings);
        (0..count)
            .map(|level| bloom_mip_extent_for(size.0, size.1, halvings, level))
            .collect()
    };
    let at_1080 = levels((1920, 1080), BloomBase::Auto);
    assert_eq!(
        at_1080,
        [
            (960, 540),
            (480, 270),
            (240, 135),
            (120, 67),
            (60, 33),
            (30, 16)
        ]
    );
    assert_eq!(levels((3840, 2160), BloomBase::Auto), at_1080);
    assert_eq!(levels((3840, 2160), BloomBase::Half)[0], (1920, 1080));

    // One halving is the count of before the key; a quarter's ceiling sits
    // one level under a half's, and never under one level.
    for (width, height) in [(1920, 1080), (64, 64), (2, 2), (1, 1), (1024, 1024)] {
        assert_eq!(
            bloom_mip_count_for_halvings(width, height, 6, 1),
            bloom_mip_count_for_size(width, height, 6)
        );
    }
    assert_eq!(bloom_mip_count_for_halvings(64, 64, 6, 2), 4);
    assert_eq!(bloom_mip_count_for_halvings(64, 64, 6, 1), 5);
    assert_eq!(bloom_mip_count_for_halvings(4, 4, 6, 2), 1);
    assert_eq!(bloom_mip_count_for_halvings(3840, 2160, 8, 2), 8);
}
