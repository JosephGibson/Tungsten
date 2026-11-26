use super::*;

#[test]
fn slowest_system_is_the_longest_or_none() {
    assert!(
        FrameTimings::new().slowest_system().is_none(),
        "no timings: no slowest system"
    );
    let mut ft = FrameTimings::new();
    ft.system_timings = vec![
        ("a".to_string(), 1.0),
        ("b".to_string(), 5.0),
        ("c".to_string(), 2.0),
    ];
    let (name, ms) = ft
        .slowest_system()
        .expect("a 1, b 5, c 2: a slowest system");
    assert_eq!(name, "b", "a 1, b 5, c 2");
    assert!((ms - 5.0).abs() < f32::EPSILON, "a 1, b 5, c 2: {ms} ms");
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
