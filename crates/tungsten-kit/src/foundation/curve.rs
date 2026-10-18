//! Curves: piecewise-linear maps from one number to another, the `curves`
//! data kind that an effect scales by and a level climbs.

use serde::{Deserialize, Serialize};
use tungsten_core::World;

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, describe_kind, describe_shape, round_trip,
};

/// The data kind of curves, and the shape of an entry.
pub(crate) const CURVES: &str = "curves";
const CURVE: &str = "curve";

/// A piecewise-linear map from `x` to `y`: points with strictly increasing
/// `x`, linear between them and held at the ends. An effect's `scale`
/// reads one to turn a request's scale into its factor, and a `level`
/// component reads one as the total count each level needs.
///
/// Data form, an entry of the `curves` kind or a curve in place:
/// `[[1, 0], [2, 10], [3, 25]]`, at least one point, every number finite,
/// the `x`s strictly increasing.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Curve;
///
/// let xp = Curve::new([(1.0, 0.0), (2.0, 10.0), (3.0, 25.0)]);
/// assert_eq!(xp.at(2.0), 10.0);
/// assert_eq!(xp.at(2.5), 17.5, "linear between points");
/// assert_eq!((xp.at(0.0), xp.at(9.0)), (0.0, 25.0), "held at the ends");
/// assert_eq!(xp.points().len(), 3);
///
/// let read: Curve = serde_json::from_str("[[0, 1], [1, 2]]").unwrap();
/// assert_eq!(read, Curve::new([(0.0, 1.0), (1.0, 2.0)]));
/// assert!(serde_json::from_str::<Curve>("[[1, 0], [1, 2]]").is_err(), "x must increase");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<[f32; 2]>", into = "Vec<[f32; 2]>")]
pub struct Curve {
    points: Vec<(f32, f32)>,
}

/// `points` at least one, every number finite, the `x`s strictly
/// increasing.
fn check_points(points: &[(f32, f32)]) -> Result<(), String> {
    if points.is_empty() {
        return Err("a curve holds at least one point".to_string());
    }
    for (i, &(x, y)) in points.iter().enumerate() {
        if !(x.is_finite() && y.is_finite()) {
            return Err(format!("{i}: must be finite (got [{x}, {y}])"));
        }
        if i > 0 && points[i - 1].0 >= x {
            return Err(format!(
                "{i}: x must be above the point before it ({} >= {x})",
                points[i - 1].0
            ));
        }
    }
    Ok(())
}

impl TryFrom<Vec<[f32; 2]>> for Curve {
    type Error = String;

    fn try_from(points: Vec<[f32; 2]>) -> Result<Self, Self::Error> {
        let points: Vec<(f32, f32)> = points.into_iter().map(|[x, y]| (x, y)).collect();
        check_points(&points)?;
        Ok(Self { points })
    }
}

impl From<Curve> for Vec<[f32; 2]> {
    fn from(curve: Curve) -> Self {
        curve.points.into_iter().map(|(x, y)| [x, y]).collect()
    }
}

impl Curve {
    /// A curve through `points`, in order.
    ///
    /// # Panics
    ///
    /// When there is no point, a number is not finite, or an `x` is not
    /// above the one before it.
    #[must_use]
    #[track_caller]
    pub fn new(points: impl IntoIterator<Item = (f32, f32)>) -> Self {
        let points: Vec<(f32, f32)> = points.into_iter().collect();
        if let Err(error) = check_points(&points) {
            panic!("Curve::new: {error}");
        }
        Self { points }
    }

    /// `y` at `x`: linear between the two points around it, the first
    /// point's `y` below the first `x` and the last point's above the last
    /// (a NaN reads as below the first).
    #[must_use]
    pub fn at(&self, x: f32) -> f32 {
        let points = &self.points;
        let (first, last) = (points[0], points[points.len() - 1]);
        if x.is_nan() || x <= first.0 {
            return first.1;
        }
        if x >= last.0 {
            return last.1;
        }
        // The first point whose `x` is above `x`; `x` lies in `[a.x, b.x)`.
        let after = points.partition_point(|point| point.0 <= x);
        let (a, b) = (points[after - 1], points[after]);
        let t = (f64::from(x) - f64::from(a.0)) / (f64::from(b.0) - f64::from(a.0));
        (f64::from(a.1) + (f64::from(b.1) - f64::from(a.1)) * t) as f32
    }

    /// The points, in `x` order.
    #[must_use]
    pub fn points(&self) -> &[(f32, f32)] {
        &self.points
    }

    /// The least `y` of its points, which is the least value it ever
    /// takes.
    pub(crate) fn min_y(&self) -> f32 {
        self.points
            .iter()
            .map(|point| point.1)
            .fold(f32::INFINITY, f32::min)
    }
}

/// Registers the `curves` data kind and its descriptions, once: whichever
/// of the effects and the level plugins builds first.
pub(crate) fn register_curves(world: &mut World) {
    world.register_data_kind::<Curve>(CURVES);
    describe_shape(
        world,
        ComponentDescription::new(
            CURVE,
            "A piecewise-linear map: linear between its points and held at the ends.",
        )
        .field(FieldDescription::new(
            "points",
            FieldType::List(Box::new(FieldType::Vec2)),
            "the value itself: `[x, y]` points, at least one, every number finite, the `x`s strictly increasing",
        ))
        .sample("[[0, 0], [1, 1]]")
        .roundtrip(round_trip::<Curve>),
    );
    describe_kind(
        world,
        CURVES,
        "A `curve`: what an effect's `scale` reads and a `level` climbs.",
    );
}

#[cfg(test)]
#[path = "../tests/curve.rs"]
mod tests;
