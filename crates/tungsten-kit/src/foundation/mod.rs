//! The kit's foundations: the phases every kit system joins, the plugin
//! set, tags and the spawn chain's origin, stats, curves, the random
//! stream, the event trace, and the field checks the scene components
//! share.

pub(crate) mod curve;
pub(crate) mod debug;
pub(crate) mod describe;
pub(crate) mod inspect;
pub(crate) mod phases;
pub(crate) mod plugins;
pub(crate) mod rng;
pub(crate) mod source;
pub(crate) mod stats;
pub(crate) mod tags;
pub(crate) mod trace;

use glam::Vec2;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A finite number, for a scene field's `deserialize_with`.
pub(crate) fn finite<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(D::Error::custom(format!("must be finite (got {value})")))
    }
}

/// A finite number above 0, for a scene field's `deserialize_with`.
pub(crate) fn positive<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "must be finite and > 0 (got {value})"
        )))
    }
}

/// A finite number of at least 0, for a scene field's `deserialize_with`.
pub(crate) fn non_negative<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "must be finite and >= 0 (got {value})"
        )))
    }
}

/// An optional finite number of at least 0, for a scene field's
/// `deserialize_with`.
pub(crate) fn optional_non_negative<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f32>, D::Error> {
    let value = Option::<f32>::deserialize(deserializer)?;
    match value {
        Some(value) if !(value.is_finite() && value >= 0.0) => Err(D::Error::custom(format!(
            "must be finite and >= 0 (got {value})"
        ))),
        value => Ok(value),
    }
}

/// An optional finite number above 0, for a scene field's
/// `deserialize_with`.
pub(crate) fn optional_positive<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f32>, D::Error> {
    let value = Option::<f32>::deserialize(deserializer)?;
    match value {
        Some(value) if !(value.is_finite() && value > 0.0) => Err(D::Error::custom(format!(
            "must be finite and > 0 (got {value})"
        ))),
        value => Ok(value),
    }
}

/// A number in `[0, 1)`, for a scene field's `deserialize_with`.
pub(crate) fn below_one<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if (0.0..1.0).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!("must be in [0, 1) (got {value})")))
    }
}

/// A number in `[0, 1]`, for a scene field's `deserialize_with`.
pub(crate) fn fraction<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!("must be in [0, 1] (got {value})")))
    }
}

/// A finite RGBA colour, for a scene field's `deserialize_with`.
pub(crate) fn finite_color<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<[f32; 4], D::Error> {
    let color = <[f32; 4]>::deserialize(deserializer)?;
    if color.iter().all(|channel| channel.is_finite()) {
        Ok(color)
    } else {
        Err(D::Error::custom(format!("must be finite (got {color:?})")))
    }
}

/// A `Vec2` written as `[x, y]`, for a `#[serde(with = "vec2")]` field.
pub(crate) mod vec2 {
    use super::{Deserialize, Deserializer, Serialize, Serializer, Vec2};

    // The reference is serde's `with` signature.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(crate) fn serialize<S: Serializer>(value: &Vec2, serializer: S) -> Result<S::Ok, S::Error> {
        [value.x, value.y].serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec2, D::Error> {
        let [x, y] = <[f32; 2]>::deserialize(deserializer)?;
        Ok(Vec2::new(x, y))
    }
}

/// A finite `Vec2` written as `[x, y]`, for a
/// `#[serde(with = "finite_vec2")]` field of a scene component.
pub(crate) mod finite_vec2 {
    use serde::de::Error as _;

    use super::{Deserialize, Deserializer, Vec2};

    pub(crate) use super::vec2::serialize;

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec2, D::Error> {
        let [x, y] = <[f32; 2]>::deserialize(deserializer)?;
        let value = Vec2::new(x, y);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(D::Error::custom(format!("must be finite (got [{x}, {y}])")))
        }
    }
}

/// An `Option<Vec2>` written as `[x, y]` or `null`, for a
/// `#[serde(with = "optional_vec2")]` field.
pub(crate) mod optional_vec2 {
    use super::{Deserialize, Deserializer, Serialize, Serializer, Vec2};

    // The reference is serde's `with` signature.
    #[allow(clippy::ref_option)]
    pub(crate) fn serialize<S: Serializer>(
        value: &Option<Vec2>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(|v| [v.x, v.y]).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Vec2>, D::Error> {
        Ok(Option::<[f32; 2]>::deserialize(deserializer)?.map(|[x, y]| Vec2::new(x, y)))
    }
}
