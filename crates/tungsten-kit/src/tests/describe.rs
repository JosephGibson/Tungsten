use std::collections::BTreeSet;

use super::*;
use serde_json::{Value, json};
use tungsten_core::{ComponentRegistry, DataRegistry, Schedule};

use crate::KitPlugins;

/// A World with every kit plugin built.
fn world() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    world
}

/// Numbers by value, the rest structurally.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        // The DTOs hold `f32`s, which serialize through `f64`.
        (Value::Number(a), Value::Number(b)) => {
            a.as_f64().map(|v| v as f32) == b.as_f64().map(|v| v as f32)
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn sample_of(description: &ComponentDescription) -> Value {
    let json = description
        .sample_json()
        .unwrap_or_else(|| panic!("{}: no sample", description.name()));
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{}: sample: {e}", description.name()))
}

fn round_trip_of(description: &ComponentDescription, value: Value) -> Result<Value, String> {
    description
        .round_trip(value)
        .unwrap_or_else(|| panic!("{}: no round-trip hook", description.name()))
}

/// The sample with `field` set to `value`; `None` when the sample is not
/// an object.
fn with_field(sample: &Value, field: &str, value: Value) -> Option<Value> {
    let mut object = sample.as_object()?.clone();
    object.insert(field.to_string(), value);
    Some(Value::Object(object))
}

#[test]
fn every_registered_scene_name_and_kind_is_described_and_nothing_else() {
    let world = world();
    let descriptions = world.resource::<KitDescriptions>();
    let described: BTreeSet<&str> = descriptions
        .iter()
        .map(ComponentDescription::name)
        .collect();
    let registered: BTreeSet<&str> = world.resource::<ComponentRegistry>().names().collect();
    assert_eq!(described, registered);
    let kinds: BTreeSet<&str> = descriptions.kinds().map(|(kind, _)| kind).collect();
    let registered_kinds: BTreeSet<&str> = world.resource::<DataRegistry>().kinds().collect();
    assert_eq!(kinds, registered_kinds);
    assert_eq!(described.len(), 13);
}

#[test]
fn each_sample_round_trips_to_its_described_defaults() {
    let world = world();
    for description in world.resource::<KitDescriptions>().iter() {
        let name = description.name();
        let sample = sample_of(description);
        let out = round_trip_of(description, sample.clone())
            .unwrap_or_else(|e| panic!("{name}: the sample does not round-trip: {e}"));
        for field in description.fields() {
            let Some(default) = field.default_json() else {
                if let Some(object) = sample.as_object() {
                    assert!(
                        object.contains_key(field.name()) || name == "despawn_outside",
                        "{name}: the sample leaves out the required `{}`",
                        field.name()
                    );
                }
                continue;
            };
            let expected: Value = serde_json::from_str(default)
                .unwrap_or_else(|e| panic!("{name}.{}: default: {e}", field.name()));
            let got = out.get(field.name()).cloned().unwrap_or(Value::Null);
            assert!(
                same(&got, &expected),
                "{name}.{}: the DTO's default {got} differs from the described {expected}",
                field.name()
            );
        }
    }
}

#[test]
fn each_described_enum_variant_deserializes() {
    let world = world();
    let mut variants = 0;
    for description in world.resource::<KitDescriptions>().iter() {
        let sample = sample_of(description);
        for field in description.fields() {
            let FieldType::Enum(names) = field.ty() else {
                continue;
            };
            for variant in *names {
                let value = with_field(&sample, field.name(), json!(variant)).unwrap();
                round_trip_of(description, value).unwrap_or_else(|e| {
                    panic!("{}.{}: `{variant}`: {e}", description.name(), field.name())
                });
                variants += 1;
            }
            let value = with_field(&sample, field.name(), json!("no_such_variant")).unwrap();
            assert!(round_trip_of(description, value).is_err());
        }
    }
    assert_eq!(
        variants,
        3 + EASING_NAMES.len(),
        "the path follower's modes and easings"
    );
}

#[test]
fn numeric_fields_take_their_extremes_and_refuse_beyond() {
    let world = world();
    let mut checked = 0;
    for description in world.resource::<KitDescriptions>().iter() {
        let name = description.name();
        let sample = sample_of(description);
        for field in description.fields() {
            let accept = |value: Value| {
                let value = with_field(&sample, field.name(), value).unwrap();
                round_trip_of(description, value.clone())
                    .unwrap_or_else(|e| panic!("{name}.{}: refused {value}: {e}", field.name()));
            };
            let refuse = |value: Value| {
                let value = with_field(&sample, field.name(), value).unwrap();
                assert!(
                    round_trip_of(description, value.clone()).is_err(),
                    "{name}.{}: took {value}",
                    field.name()
                );
            };
            match *field.ty() {
                FieldType::F32 { min, max } => {
                    match min {
                        Bound::Open => accept(json!(-1e6)),
                        Bound::Inclusive(v) => {
                            accept(json!(v));
                            refuse(json!(v - 1.0));
                        }
                        Bound::Exclusive(v) => {
                            refuse(json!(v));
                            accept(json!(v + 1.0));
                        }
                    }
                    match max {
                        Bound::Open => accept(json!(1e6)),
                        Bound::Inclusive(v) => {
                            accept(json!(v));
                            refuse(json!(v + 1.0));
                        }
                        Bound::Exclusive(v) => {
                            refuse(json!(v));
                            accept(json!(v - 0.5));
                        }
                    }
                    refuse(json!("text"));
                    checked += 1;
                }
                FieldType::U32 { min, max } => {
                    match min {
                        None => accept(json!(0)),
                        Some(v) => {
                            accept(json!(v));
                            if v > 0 {
                                refuse(json!(v - 1));
                            }
                        }
                    }
                    match max {
                        None => accept(json!(u32::MAX)),
                        Some(v) => {
                            accept(json!(v));
                            refuse(json!(u64::from(v) + 1));
                        }
                    }
                    refuse(json!(-1));
                    checked += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(checked, 15, "the kit's numeric fields");
}

#[test]
fn the_snapshot_renders_one_line_per_component_and_field() {
    let world = world();
    let text = world.resource::<KitDescriptions>().render();
    let components = text.lines().filter(|l| l.starts_with("component ")).count();
    let fields = text.lines().filter(|l| l.starts_with("  field ")).count();
    let kinds = text.lines().filter(|l| l.starts_with("kind ")).count();
    assert_eq!(components, 13);
    assert_eq!(kinds, 1);
    assert!(fields >= 20, "{fields}");
    assert!(text.contains("component health: "));
    assert!(text.contains("  field invulnerability: f32 (>= 0) = 0.0 "));
    assert!(text.contains("  field jump_cut: f32 (>= 0, <= 1) = 0.5 "));
    assert!(text.contains("kind tags: "));
    assert!(text.lines().all(|l| !l.ends_with(' ')));
}

#[test]
fn a_description_added_twice_panics_and_lookups_find_it() {
    let mut descriptions = KitDescriptions::new();
    descriptions.add(ComponentDescription::new("torch", "A light."));
    descriptions.add_kind("torches", "Lights.");
    assert_eq!(descriptions.get("torch").unwrap().summary(), "A light.");
    assert!(descriptions.get("lamp").is_none());
    assert_eq!(descriptions.kinds().count(), 1);
    let twice = std::panic::catch_unwind(move || {
        descriptions.add(ComponentDescription::new("torch", "Again."));
    });
    assert!(twice.is_err());
}
