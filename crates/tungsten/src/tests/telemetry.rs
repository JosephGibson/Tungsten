use super::*;

#[test]
fn default_is_zero() {
    let ft = FrameTimings::new();
    assert_eq!(ft.update_ms, 0.0);
    assert_eq!(ft.render_ms, 0.0);
    assert_eq!(ft.render_acquire_ms, 0.0);
    assert_eq!(ft.render_encode_ms, 0.0);
    assert_eq!(ft.render_submit_present_ms, 0.0);
    assert_eq!(ft.flush_ms, 0.0);
    assert!(ft.system_timings.is_empty());
}

#[test]
fn slowest_system_empty() {
    assert!(FrameTimings::new().slowest_system().is_none());
}

#[test]
fn slowest_system_finds_max() {
    let mut ft = FrameTimings::new();
    ft.system_timings = vec![
        ("a".to_string(), 1.0),
        ("b".to_string(), 5.0),
        ("c".to_string(), 2.0),
    ];
    let (name, ms) = ft.slowest_system().unwrap();
    assert_eq!(name, "b");
    assert!((ms - 5.0).abs() < f32::EPSILON);
}
