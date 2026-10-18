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

/// Every component description, then every shape: what the description
/// tests hold to their DTOs.
fn described(descriptions: &KitDescriptions) -> impl Iterator<Item = &ComponentDescription> {
    descriptions.iter().chain(descriptions.shapes())
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

/// The value that sets `field` to `value`: the sample with it set, or, for
/// a shape that holds exactly one field, that field alone. `None` when the
/// sample is not an object.
fn with_field(description: &ComponentDescription, field: &str, value: Value) -> Option<Value> {
    let mut object = if description.is_one_of() {
        serde_json::Map::new()
    } else {
        sample_of(description).as_object()?.clone()
    };
    object.insert(field.to_string(), value);
    Some(Value::Object(object))
}

/// The sample without the fields that have defaults, with `field` set to
/// `value`: a whole-number probe, which may not sit beside an exclusive
/// sibling such as a counter's `add` and `set`.
fn required_with(description: &ComponentDescription, field: &str, value: Value) -> Value {
    let mut object = sample_of(description).as_object().unwrap().clone();
    for other in description.fields() {
        if other.default_json().is_some() {
            object.remove(other.name());
        }
    }
    object.insert(field.to_string(), value);
    Value::Object(object)
}

/// Every shape a field type names, through lists.
fn shapes_named(ty: &FieldType, out: &mut Vec<&'static str>) {
    match ty {
        FieldType::Shape(name) => out.push(name),
        FieldType::List(inner) => shapes_named(inner, out),
        _ => {}
    }
}

/// A value of `ty` the DTOs take, for the one-of probe.
fn valid_value(descriptions: &KitDescriptions, ty: &FieldType) -> Value {
    match ty {
        FieldType::Bool => json!(true),
        FieldType::F32 { min, .. } => match min {
            Bound::Inclusive(v) => json!(v),
            Bound::Exclusive(v) => json!(v + 1.0),
            Bound::Open => json!(1.0),
        },
        FieldType::Enum(names) => json!(names[0]),
        // A data field takes an entry's ID.
        FieldType::Prefab | FieldType::Text | FieldType::Tag | FieldType::Data(_) => {
            json!("puff")
        }
        FieldType::Vec2 => json!([1.0, 2.0]),
        // A target's `source` takes the empty object.
        FieldType::Object(_) => json!({}),
        FieldType::Shape(name) => sample_of(descriptions.get_shape(name).unwrap()),
        other => panic!("no probe value for {other:?}"),
    }
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
    assert_eq!(described.len(), 41);
    assert_eq!(kinds.len(), 5);
    assert_eq!(descriptions.shapes().count(), 19);
}

#[test]
fn every_shape_a_field_names_is_described() {
    let world = world();
    let descriptions = world.resource::<KitDescriptions>();
    let mut named = Vec::new();
    for description in described(descriptions) {
        for field in description.fields() {
            shapes_named(field.ty(), &mut named);
        }
    }
    for name in &named {
        assert!(
            descriptions.get_shape(name).is_some(),
            "a field names the shape `{name}`, which nothing describes"
        );
    }
    let used: BTreeSet<&str> = named.into_iter().collect();
    for shape in descriptions.shapes() {
        // A kind's entry, which a field names through its kind.
        assert!(
            used.contains(shape.name())
                || ["cue_rule", "damage_type", "stat_bounds", "weighted_table"]
                    .contains(&shape.name()),
            "the shape `{}` is described but no field names it",
            shape.name()
        );
    }
}

#[test]
fn each_sample_round_trips_to_its_described_defaults() {
    let world = world();
    for description in described(world.resource::<KitDescriptions>()) {
        let name = description.name();
        let sample = sample_of(description);
        let out = round_trip_of(description, sample.clone())
            .unwrap_or_else(|e| panic!("{name}: the sample does not round-trip: {e}"));
        for field in description.fields() {
            let Some(default) = field.default_json() else {
                if let Some(object) = sample.as_object()
                    && !description.is_one_of()
                {
                    // `despawn_outside` takes one of two; an `Object`
                    // field is the value itself.
                    assert!(
                        object.contains_key(field.name())
                            || name == "despawn_outside"
                            || matches!(field.ty(), FieldType::Object(_)),
                        "{name}: the sample leaves out the required `{}`",
                        field.name()
                    );
                }
                continue;
            };
            // A field the sample gives is no default's to check: a
            // counter action names one of `add` and `set`.
            if sample.get(field.name()).is_some() {
                continue;
            }
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
fn a_one_of_shape_takes_each_field_alone_and_refuses_two() {
    let world = world();
    let descriptions = world.resource::<KitDescriptions>();
    let mut probed = 0;
    for shape in descriptions.shapes().filter(|shape| shape.is_one_of()) {
        let fields = shape.fields();
        for field in fields {
            let value = json!({ field.name(): valid_value(descriptions, field.ty()) });
            round_trip_of(shape, value.clone())
                .unwrap_or_else(|e| panic!("{}: {value} alone: {e}", shape.name()));
            probed += 1;
        }
        for pair in fields.windows(2) {
            let value = json!({
                pair[0].name(): valid_value(descriptions, pair[0].ty()),
                pair[1].name(): valid_value(descriptions, pair[1].ty()),
            });
            assert!(
                round_trip_of(shape, value.clone()).is_err(),
                "{}: took two fields: {value}",
                shape.name()
            );
        }
        assert!(
            round_trip_of(shape, json!({})).is_err(),
            "{}: took none",
            shape.name()
        );
    }
    assert_eq!(
        probed, 15,
        "the cue action's ten forms, the target's three and the hit shape's two"
    );
}

#[test]
fn each_described_enum_variant_deserializes() {
    let world = world();
    let mut variants = 0;
    for description in described(world.resource::<KitDescriptions>()) {
        for field in description.fields() {
            let FieldType::Enum(names) = field.ty() else {
                continue;
            };
            for variant in *names {
                let value = with_field(description, field.name(), json!(variant)).unwrap();
                round_trip_of(description, value).unwrap_or_else(|e| {
                    panic!("{}.{}: `{variant}`: {e}", description.name(), field.name())
                });
                variants += 1;
            }
            let value = with_field(description, field.name(), json!("no_such_variant")).unwrap();
            assert!(round_trip_of(description, value).is_err());
        }
    }
    assert_eq!(
        variants,
        3 + EASING_NAMES.len() + 4 + 3 + 3 + 2 + 3 + 4 + 3 + 4 + 3,
        "the path follower's modes and easings, the squash triggers, the action targets, the \
         steering's modes, the aim's and the facing's sources, the facing's snaps and the wrap's \
         axes"
    );
}

#[test]
fn numeric_fields_take_their_extremes_and_refuse_beyond() {
    let world = world();
    let mut checked = 0;
    for description in described(world.resource::<KitDescriptions>()) {
        let name = description.name();
        for field in description.fields() {
            let accept = |value: Value| {
                let value = with_field(description, field.name(), value).unwrap();
                round_trip_of(description, value.clone())
                    .unwrap_or_else(|e| panic!("{name}.{}: refused {value}: {e}", field.name()));
            };
            let refuse = |value: Value| {
                let value = with_field(description, field.name(), value).unwrap();
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
                FieldType::I64 { min, max } => {
                    let probe = |value: Value| required_with(description, field.name(), value);
                    let accepts = |value: Value| {
                        let value = probe(value);
                        round_trip_of(description, value.clone()).unwrap_or_else(|e| {
                            panic!("{name}.{}: refused {value}: {e}", field.name())
                        });
                    };
                    let refuses = |value: Value| {
                        let value = probe(value);
                        assert!(
                            round_trip_of(description, value.clone()).is_err(),
                            "{name}.{}: took {value}",
                            field.name()
                        );
                    };
                    accepts(json!(min.unwrap_or(i64::MIN)));
                    accepts(json!(max.unwrap_or(i64::MAX)));
                    if max.is_none() {
                        refuses(json!(u64::MAX));
                    }
                    refuses(json!(0.5));
                    refuses(json!("text"));
                    checked += 1;
                }
                FieldType::Damage => {
                    accept(json!(0.0));
                    refuse(json!(-1.0));
                    accept(json!(1e6));
                    refuse(json!("text"));
                    accept(json!({ "damage": 1.0, "damage.fire": 0.0 }));
                    accept(json!({ "damage.fire": 1e6 }));
                    refuse(json!({ "damage.fire": -1.0 }));
                    refuse(json!({}));
                    refuse(json!({ "damage.fire": 0.0 }));
                    checked += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        checked, 70,
        "the kit's numeric fields, two of them a hit's number-or-map damage"
    );
}

#[test]
fn the_snapshot_renders_one_line_per_component_shape_and_field() {
    let world = world();
    let text = world.resource::<KitDescriptions>().render();
    let count = |prefix: &str| text.lines().filter(|l| l.starts_with(prefix)).count();
    assert_eq!(count("component "), 41);
    assert_eq!(count("shape "), 19);
    assert_eq!(count("kind "), 5);
    assert!(count("  field ") >= 40, "{}", count("  field "));
    assert!(text.contains("component health: "));
    assert!(text.contains("  field invulnerability: f32 (>= 0) = 0.0 "));
    assert!(text.contains("  field jump_cut: f32 (>= 0, <= 1) = 0.5 "));
    assert!(text.contains("  field damaged: list of shape cue_action = [] "));
    assert!(text.contains("  field add: i64 = null "));
    assert!(
        text.contains("shape cue_action: One reaction to an event.\n  exactly one of its fields\n")
    );
    assert!(text.contains("kind tags: "));
    assert!(text.contains("  field roll: loot_tables ID or entry "));
    assert!(text.contains("shape hit_shape: "));
    let first_shape = text.find("\nshape ").unwrap();
    assert!(text.rfind("\ncomponent ").unwrap() < first_shape);
    assert!(first_shape < text.find("\nkind ").unwrap());
    assert!(text.lines().all(|l| !l.ends_with(' ')));
}

#[test]
fn a_description_added_twice_panics_and_lookups_find_it() {
    let mut descriptions = KitDescriptions::new();
    descriptions.add(ComponentDescription::new("torch", "A light."));
    descriptions.add_shape(ComponentDescription::new("flame", "A flame.").one_of());
    descriptions.add_kind("torches", "Lights.");
    assert_eq!(descriptions.get("torch").unwrap().summary(), "A light.");
    assert!(descriptions.get("lamp").is_none());
    assert!(descriptions.get_shape("flame").unwrap().is_one_of());
    assert!(
        descriptions.get("flame").is_none(),
        "a shape is no component"
    );
    assert_eq!(descriptions.kinds().count(), 1);
    let twice = std::panic::catch_unwind(move || {
        descriptions.add_shape(ComponentDescription::new("flame", "Again."));
    });
    assert!(twice.is_err());
    let mut descriptions = KitDescriptions::new();
    descriptions.add(ComponentDescription::new("torch", "A light."));
    let twice = std::panic::catch_unwind(move || {
        descriptions.add(ComponentDescription::new("torch", "Again."));
    });
    assert!(twice.is_err());
}
