use super::*;
use serde_json::json;
use tungsten_core::DataRegistry;

#[test]
fn a_curve_is_linear_between_its_points_and_held_at_its_ends() {
    let xp = Curve::new([(1.0, 0.0), (2.0, 10.0), (4.0, 50.0)]);
    assert_eq!(xp.at(1.0), 0.0);
    assert_eq!(xp.at(1.5), 5.0);
    assert_eq!(xp.at(2.0), 10.0);
    assert_eq!(xp.at(3.0), 30.0);
    assert_eq!(xp.at(4.0), 50.0);
    assert_eq!(
        (xp.at(-7.0), xp.at(0.999), xp.at(4.001), xp.at(1e9)),
        (0.0, 0.0, 50.0, 50.0)
    );
    assert_eq!(xp.at(f32::NAN), 0.0, "a NaN reads as below the first point");
    let one = Curve::new([(3.0, 7.0)]);
    assert_eq!((one.at(-1.0), one.at(3.0), one.at(9.0)), (7.0, 7.0, 7.0));
    assert_eq!(one.min_y(), 7.0);
    assert_eq!(
        Curve::new([(0.0, 2.0), (1.0, -1.0), (2.0, 5.0)]).min_y(),
        -1.0
    );
}

#[test]
fn a_curve_refuses_no_point_a_non_increasing_x_and_a_non_finite_number() {
    for (bad, error) in [
        (json!([]), "at least one point"),
        (
            json!([[1, 0], [1, 2]]),
            "1: x must be above the point before it (1 >= 1)",
        ),
        (
            json!([[2, 0], [1, 2]]),
            "1: x must be above the point before it (2 >= 1)",
        ),
        (json!([[0, 1e308]]), "0: must be finite"),
    ] {
        let got = serde_json::from_value::<Curve>(bad.clone())
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(got.contains(error), "{bad}: {got}");
    }
    assert!(std::panic::catch_unwind(|| Curve::new([(1.0, 0.0), (0.5, 1.0)])).is_err());
    assert!(std::panic::catch_unwind(|| Curve::new([(f32::INFINITY, 0.0)])).is_err());
    assert!(std::panic::catch_unwind(|| Curve::new(std::iter::empty())).is_err());
}

#[test]
fn a_curve_round_trips_as_its_points() {
    let curve = Curve::new([(0.0, 0.5), (1.0, 1.0), (2.0, 2.0)]);
    let data = serde_json::to_value(&curve).unwrap();
    assert_eq!(data, json!([[0.0, 0.5], [1.0, 1.0], [2.0, 2.0]]));
    assert_eq!(serde_json::from_value::<Curve>(data).unwrap(), curve);
    assert_eq!(curve.points(), [(0.0, 0.5), (1.0, 1.0), (2.0, 2.0)]);
}

#[test]
fn the_curves_kind_registers_once_from_two_plugins() {
    let mut world = World::new();
    register_curves(&mut world);
    register_curves(&mut world);
    let data = world.resource::<DataRegistry>();
    assert!(data.contains_kind(CURVES));
    assert_eq!(data.kind_of::<Curve>(), Some(CURVES));
    let descriptions = world.resource::<crate::KitDescriptions>();
    assert_eq!(
        descriptions.shapes().filter(|s| s.name() == CURVE).count(),
        1
    );
    assert_eq!(
        descriptions.kinds().filter(|(k, _)| *k == CURVES).count(),
        1
    );
}
