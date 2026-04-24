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
