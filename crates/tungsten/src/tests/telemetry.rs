use super::*;

#[test]
fn default_is_zero() {
    let ft = FrameTimings::new();
    assert_eq!(ft.update_ms, 0.0);
    assert_eq!(ft.render_ms, 0.0);
    assert_eq!(ft.render_acquire_ms, 0.0);
    assert_eq!(ft.render_encode_ms, 0.0);
    assert_eq!(ft.render_encode_pre_acquire_ms, 0.0);
    assert_eq!(ft.render_submit_present_ms, 0.0);
    assert_eq!(ft.flush_ms, 0.0);
    assert!(ft.interval_ms.is_none());
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

/// Once warm, a system adds no allocation to a frame (debug-pass): its
/// timing entry and its name are the frame before last's, reused.
#[test]
fn a_system_adds_no_allocation_to_a_warm_frame() {
    use crate::App;
    use crate::testing::{CounterGuard, Harness};
    use tungsten_core::{Config, World};

    let window = |extra: usize| {
        let mut app = App::new(Config::default()).unwrap();
        for _ in 0..extra {
            app.add_system(|_: &mut World| {});
        }
        CounterGuard::measure(&mut Harness::new(app), 30, 60).allocations
    };
    assert_eq!(window(8), window(0), "eight more no-op systems");
}
