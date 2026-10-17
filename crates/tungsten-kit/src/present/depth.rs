//! Depth sort: a sprite's draw order taken from how low it stands, so what
//! stands lower draws in front.

use serde::{Deserialize, Serialize};
use tungsten_core::{Entity, Plugin, Schedule, SpawnContext, Sprite, Transform, World, system};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, TOP_DOWN, describe, round_trip,
};
use crate::foundation::finite;
use crate::foundation::inspect::register_rows_with;
use crate::foundation::phases::{Phase, ensure_phases};

/// [`DepthSortPlugin`]'s system, in the present phase.
pub const DEPTH_SORT: &str = "depth_sort";

/// The scene component the plugin registers.
const DEPTH_SORT_COMPONENT: &str = "depth_sort";

/// Sorts its entity's sprite by how low it stands: in a top-down or grid
/// view, a character in front of a crate above it and behind one below.
/// Each frame [`DepthSortPlugin`]'s system sets
/// `Sprite.z_order = base + round(Transform.position.y + offset)`, clamped to
/// the `i32` range, so what stands lower draws later; the default extract
/// sorts by `(z_order, entity)` as ever. The offset is the feet's distance
/// below the drawn point, the sprite's top-left, such as its height; the
/// base lifts a layer of sorted sprites over another. It needs a `Sprite`,
/// writes nothing else, and works on children.
///
/// Scene name `depth_sort`: `{ "offset": 32, "base": 0 }`, `offset` in
/// world pixels and `base` a whole number, both 0 by default. It is data: a
/// game's save can hold it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{Plugin, Schedule, Sprite, SpriteAssetId, Stage, Transform, World};
/// use tungsten_kit::{DepthSort, DepthSortPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// DepthSortPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A 32 px tall character, sorted on its feet, over the floor's layer.
/// let sort = DepthSort::new().with_offset(32.0).with_base(1000);
/// let hero = world.spawn_with((
///     sort,
///     Transform::from_position(Vec2::new(40.0, 100.4)),
///     Sprite::new(SpriteAssetId::new(0)),
/// ));
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert_eq!(world.get::<Sprite>(hero).unwrap().z_order, 1132);
///
/// let saved = serde_json::to_value(sort).unwrap();
/// assert_eq!(saved, serde_json::json!({ "offset": 32.0, "base": 1000 }));
/// assert_eq!(serde_json::from_value::<DepthSort>(saved).unwrap(), sort);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "DepthSortDef", into = "DepthSortDef")]
pub struct DepthSort {
    offset: f32,
    base: i32,
}

impl Default for DepthSort {
    fn default() -> Self {
        Self::new()
    }
}

impl DepthSort {
    /// A sort on the drawn point, from a base of 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            base: 0,
        }
    }

    /// The sort, on the point `offset` world pixels below the drawn point.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: f32) -> Self {
        assert!(
            offset.is_finite(),
            "DepthSort::with_offset: offset must be finite (got {offset})"
        );
        self.offset = offset;
        self
    }

    /// The sort, from `base`: what sorts over another layer of sorted
    /// sprites.
    #[must_use]
    pub fn with_base(mut self, base: i32) -> Self {
        self.base = base;
        self
    }
}

/// Registers the `depth_sort` scene component and [`DEPTH_SORT`] in the
/// present phase. Its system sets every [`DepthSort`] sprite's `z_order`
/// once a frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct DepthSortPlugin;

impl Plugin for DepthSortPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows_with(world, "DepthSort", |world: &World, entity: Entity| {
            let Some(sort) = world.get::<DepthSort>(entity) else {
                return Vec::new();
            };
            let z_order = world.get::<Sprite>(entity).map_or_else(
                || "no sprite".to_string(),
                |sprite| sprite.z_order.to_string(),
            );
            vec![
                ("offset", format!("{:.1}", sort.offset)),
                ("base", sort.base.to_string()),
                ("z_order", z_order),
            ]
        });
        describe(
            world,
            ComponentDescription::new(
                DEPTH_SORT_COMPONENT,
                "Sorts the entity's sprite by how low it stands: z_order = base + round(y + offset), so what stands lower draws in front.",
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::F32 {
                        min: Bound::Open,
                        max: Bound::Open,
                    },
                    "world pixels from the drawn point down to the feet, such as the sprite's height",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "base",
                    FieldType::I64 {
                        min: Some(i64::from(i32::MIN)),
                        max: Some(i64::from(i32::MAX)),
                    },
                    "added to the sort, lifting a layer over another",
                )
                .default("0"),
            )
            .genre(TOP_DOWN)
            .genre(GRID)
            .sample("{}")
            .roundtrip(round_trip::<DepthSortDef>),
        );
        world.register_component_with(
            DEPTH_SORT_COMPONENT,
            |def: DepthSortDef, _: &SpawnContext<'_>| {
                (DepthSort::new().with_offset(def.offset).with_base(def.base),)
            },
        );
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(DEPTH_SORT, depth_sort)),
        );
    }
}

/// `components.depth_sort` as a scene and a save write it.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DepthSortDef {
    #[serde(default, deserialize_with = "finite")]
    offset: f32,
    #[serde(default)]
    base: i32,
}

impl From<DepthSortDef> for DepthSort {
    fn from(def: DepthSortDef) -> Self {
        Self {
            offset: def.offset,
            base: def.base,
        }
    }
}

impl From<DepthSort> for DepthSortDef {
    fn from(sort: DepthSort) -> Self {
        Self {
            offset: sort.offset,
            base: sort.base,
        }
    }
}

/// Sets every depth-sorted sprite's `z_order` from how low it stands.
fn depth_sort(world: &mut World) {
    for (sort, transform, sprite) in world.query_mut::<(&DepthSort, &Transform, &mut Sprite)>() {
        let depth = f64::from(sort.base)
            + (f64::from(transform.position.y) + f64::from(sort.offset)).round();
        sprite.z_order = depth.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    }
}

#[cfg(test)]
#[path = "../tests/depth.rs"]
mod tests;
