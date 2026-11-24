use super::*;

#[test]
fn auto_present_mode_preserves_vsync_selection() {
    let supported = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];

    assert_eq!(
        resolve_present_mode(&supported, None, true).unwrap(),
        wgpu::PresentMode::Fifo
    );
    assert_eq!(
        resolve_present_mode(&supported, Some(PresentModeConfig::Auto), false).unwrap(),
        wgpu::PresentMode::Immediate
    );
}

#[test]
fn auto_present_mode_uses_documented_fallbacks() {
    assert_eq!(
        resolve_present_mode(&[wgpu::PresentMode::Mailbox], None, false).unwrap(),
        wgpu::PresentMode::Mailbox
    );
    assert_eq!(
        resolve_present_mode(&[wgpu::PresentMode::FifoRelaxed], None, false).unwrap(),
        wgpu::PresentMode::AutoNoVsync
    );
    assert_eq!(
        resolve_present_mode(&[wgpu::PresentMode::FifoRelaxed], None, true).unwrap(),
        wgpu::PresentMode::AutoVsync
    );
}

#[test]
fn explicit_present_mode_override_beats_vsync() {
    let supported = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];
    let chosen =
        resolve_present_mode(&supported, Some(PresentModeConfig::Immediate), true).unwrap();
    assert_eq!(chosen, wgpu::PresentMode::Immediate);
}

#[test]
fn unsupported_explicit_present_mode_returns_error() {
    let err = resolve_present_mode(
        &[wgpu::PresentMode::Fifo],
        Some(PresentModeConfig::Mailbox),
        false,
    )
    .unwrap_err();

    match err {
        RenderError::UnsupportedPresentMode {
            requested,
            available,
        } => {
            assert_eq!(requested, "mailbox");
            assert_eq!(available, vec!["fifo".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn zero_frame_latency_is_rejected() {
    let err = resolve_max_frame_latency(Some(0), wgpu::PresentMode::Fifo).unwrap_err();
    match err {
        RenderError::InvalidFrameLatency(0) => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn default_frame_latency_preserves_existing_policy() {
    assert_eq!(
        resolve_max_frame_latency(None, wgpu::PresentMode::Immediate).unwrap(),
        1
    );
    assert_eq!(
        resolve_max_frame_latency(None, wgpu::PresentMode::Mailbox).unwrap(),
        1
    );
    assert_eq!(
        resolve_max_frame_latency(None, wgpu::PresentMode::AutoNoVsync).unwrap(),
        1
    );
    assert_eq!(
        resolve_max_frame_latency(None, wgpu::PresentMode::Fifo).unwrap(),
        2
    );
}

#[test]
fn present_mode_labels_are_stable_lowercase_strings() {
    assert_eq!(present_mode_label(wgpu::PresentMode::Fifo), "fifo");
    assert_eq!(
        present_mode_label(wgpu::PresentMode::Immediate),
        "immediate"
    );
    assert_eq!(present_mode_label(wgpu::PresentMode::Mailbox), "mailbox");
    assert_eq!(
        present_mode_label(wgpu::PresentMode::AutoVsync),
        "auto_vsync"
    );
    assert_eq!(
        present_mode_label(wgpu::PresentMode::AutoNoVsync),
        "auto_no_vsync"
    );
}

#[test]
fn msaa_falls_back_to_a_count_the_device_takes() {
    use wgpu::TextureFormatFeatureFlags as F;
    let guaranteed = F::MULTISAMPLE_X4 | F::MULTISAMPLE_RESOLVE;
    let every = F::MULTISAMPLE_X2 | F::MULTISAMPLE_X4 | F::MULTISAMPLE_X8 | F::MULTISAMPLE_RESOLVE;
    // WebGPU guarantees 4 only, so 2 and 8 fall back to 4.
    assert_eq!(msaa_sample_count(8, guaranteed, Some(F::MULTISAMPLE_X4)), 4);
    assert_eq!(msaa_sample_count(2, guaranteed, Some(F::MULTISAMPLE_X4)), 4);
    assert_eq!(msaa_sample_count(8, every, Some(every)), 8);
    assert_eq!(msaa_sample_count(2, every, Some(every)), 2);
    // The depth target must take the count too, when there is one.
    assert_eq!(msaa_sample_count(8, every, Some(F::MULTISAMPLE_X4)), 4);
    assert_eq!(msaa_sample_count(8, every, None), 8);
    // The color target resolves, so it needs resolve support.
    assert_eq!(msaa_sample_count(4, F::MULTISAMPLE_X4, None), 1);
    assert_eq!(msaa_sample_count(4, F::empty(), Some(F::empty())), 1);
    assert_eq!(msaa_sample_count(1, F::empty(), Some(F::empty())), 1);
}

#[test]
fn the_scene_format_follows_render_hdr_and_what_the_device_draws() {
    use tungsten_core::config::HdrMode;
    use wgpu::TextureFormat::{Rg11b10Ufloat, Rgba16Float};
    use wgpu::TextureFormatFeatureFlags as F;
    use wgpu::TextureUsages as U;
    let drawn = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC;
    let blended = F::BLENDABLE | F::FILTERABLE;
    let resolved = blended | F::MULTISAMPLE_X4 | F::MULTISAMPLE_RESOLVE;

    // Off draws the scene in the frame's own format, whatever the device.
    assert_eq!(hdr_scene_format(HdrMode::Off, drawn, resolved, 4), None);
    assert_eq!(
        hdr_scene_format(HdrMode::Off, U::empty(), F::empty(), 1),
        None
    );
    // `rgba16f` is drawn, blended, filtered and resolved everywhere.
    for msaa in [1, 4] {
        assert_eq!(
            hdr_scene_format(HdrMode::Rgba16f, U::empty(), F::empty(), msaa),
            Some((Rgba16Float, false))
        );
    }
    // `rg11b10` where the device draws into it, samples it and blends it.
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, drawn, blended, 1),
        Some((Rg11b10Ufloat, false))
    );
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, drawn, resolved, 4),
        Some((Rg11b10Ufloat, false))
    );
    // Elsewhere `rgba16f` stands in, and says so: no attachment, no
    // sampling, no blend, no filter, or no resolve of the scene's samples.
    let fallback = Some((Rgba16Float, true));
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, U::TEXTURE_BINDING, blended, 1),
        fallback
    );
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, U::RENDER_ATTACHMENT, blended, 1),
        fallback
    );
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, drawn, F::FILTERABLE, 1),
        fallback
    );
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, drawn, F::BLENDABLE, 1),
        fallback
    );
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, drawn, blended, 4),
        fallback
    );
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, drawn, blended | F::MULTISAMPLE_X4, 4),
        fallback
    );

    // The usages are the device's, decided as its flags are.
    let webgpu = wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT;
    let specific = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    assert_eq!(
        device_format_usages(specific, webgpu, drawn, Rg11b10Ufloat),
        drawn
    );
    assert_eq!(
        device_format_usages(
            wgpu::Features::empty(),
            wgpu::DownlevelFlags::empty(),
            drawn,
            Rg11b10Ufloat
        ),
        drawn
    );
    // A WebGPU-compliant adapter without the feature: the guarantee only,
    // under which the format is no render attachment.
    let guaranteed = device_format_usages(wgpu::Features::empty(), webgpu, drawn, Rg11b10Ufloat);
    assert!(!guaranteed.contains(U::RENDER_ATTACHMENT));
    assert_eq!(
        hdr_scene_format(HdrMode::Rg11b10, guaranteed, blended, 1),
        fallback
    );
}

#[test]
fn device_format_flags_follow_wgpu_core() {
    use wgpu::TextureFormatFeatureFlags as F;
    let format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let adapter =
        F::MULTISAMPLE_X2 | F::MULTISAMPLE_X4 | F::MULTISAMPLE_X8 | F::MULTISAMPLE_RESOLVE;
    let webgpu = wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT;
    // A WebGPU-compliant adapter without the feature: the guarantee only.
    let flags = device_format_flags(wgpu::Features::empty(), webgpu, adapter, format);
    assert!(flags.sample_count_supported(4));
    assert!(!flags.sample_count_supported(8));
    // With the feature, or on a downlevel adapter: the adapter's own.
    let specific = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    assert_eq!(
        device_format_flags(specific, webgpu, adapter, format),
        adapter
    );
    assert_eq!(
        device_format_flags(
            wgpu::Features::empty(),
            wgpu::DownlevelFlags::empty(),
            adapter,
            format
        ),
        adapter
    );
}

#[test]
fn the_surface_is_no_larger_than_the_texture_limit() {
    // Three 4K screens side by side are 11,520 px wide; the default limits
    // allow 8,192.
    assert_eq!(surface_extent((11_520, 2_160), 8_192), (8_192, 2_160));
    assert_eq!(surface_extent((1_280, 9_000), 8_192), (1_280, 8_192));
    assert_eq!(surface_extent((1_280, 720), 8_192), (1_280, 720));
    assert_eq!(surface_extent((0, 720), 8_192), (0, 720));
}
