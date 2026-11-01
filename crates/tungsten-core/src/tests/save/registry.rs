use serde::Deserialize;
use serde_json::json;

use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Health {
    current: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Mana(u32);

/// A component holding an entity, saved through a form that leaves it out.
#[derive(Debug, Clone, PartialEq)]
struct Leash {
    to: Entity,
    length: f32,
}

#[derive(Serialize, Deserialize)]
struct LeashForm {
    length: f32,
}

impl From<&Leash> for LeashForm {
    fn from(leash: &Leash) -> Self {
        Self {
            length: leash.length,
        }
    }
}

impl From<LeashForm> for Leash {
    fn from(form: LeashForm) -> Self {
        Self {
            to: Entity::PLACEHOLDER,
            length: form.length,
        }
    }
}

fn save_leash(leash: &Leash, out: &mut RefWriter<'_>) {
    out.put("to", leash.to);
}

fn load_leash(leash: &mut Leash, refs: &RefReader<'_>) {
    if let Some(to) = refs.first("to") {
        leash.to = to;
    }
}

fn leash_keys(_: &Leash) -> Vec<String> {
    vec!["to".into()]
}

fn length_in_tiles(mut value: Value) -> Result<Value, String> {
    let length = value["length"].as_f64().ok_or("no length")?;
    value["length"] = json!(length * 16.0);
    Ok(value)
}

#[test]
fn a_component_registers_in_one_line_and_twice_is_a_no_op() {
    let mut world = World::new();
    assert!(world.register_saved::<Health>("health"));
    assert!(!world.register_saved::<Health>("health"));
    assert!(world.register_saved_resource::<Mana>("mana"));
    assert!(!world.register_saved_resource::<Mana>("mana"));
    assert!(
        world.register_saved_resource::<Health>("health"),
        "a resource keeps its own names"
    );

    let registry = world.resource::<SaveRegistry>();
    assert_eq!(registry.component_names().collect::<Vec<_>>(), ["health"]);
    assert_eq!(
        registry.resource_names().collect::<Vec<_>>(),
        ["health", "mana"]
    );
    assert_eq!(registry.component_name::<Health>(), Some("health"));
    assert_eq!(registry.component_name::<Mana>(), None);
    assert_eq!(registry.resource_name::<Mana>(), Some("mana"));
    assert_eq!(registry.component_version("health"), Some(1));
    assert!(!registry.component_has_refs("health"));
}

#[test]
#[should_panic(expected = "saved component name `health` already names")]
fn a_name_taken_by_another_type_panics() {
    let mut world = World::new();
    world.register_saved::<Health>("health");
    world.register_saved::<Mana>("health");
}

#[test]
#[should_panic(expected = "is already saved as component `health`; it cannot also be `hp`")]
fn a_type_under_another_name_panics() {
    let mut world = World::new();
    world.register_saved::<Health>("health");
    world.register_saved::<Health>("hp");
}

#[test]
#[should_panic(expected = "must match [a-z][a-z0-9_]*")]
fn a_name_off_the_rule_panics() {
    World::new().register_saved::<Health>("Health");
}

#[test]
fn a_version_steps_and_a_reference_codec_register_with_the_component() {
    let mut world = World::new();
    let options = SaveOptions::<Leash>::form::<LeashForm>()
        .version(2)
        .step(1, length_in_tiles)
        .refs(RefCodec {
            save: save_leash,
            load: load_leash,
            keys: leash_keys,
        });
    assert!(world.register_saved_with("leash", options));
    let registry = world.resource::<SaveRegistry>().clone();
    assert_eq!(registry.component_version("leash"), Some(2));
    assert!(registry.component_has_refs("leash"));

    let entry = &registry.components.entries["leash"];
    assert_eq!(
        entry.migrate(1, json!({ "length": 2.0 })),
        Ok(json!({ "length": 32.0 }))
    );
    assert_eq!(
        entry.migrate(2, json!({ "length": 2.0 })),
        Ok(json!({ "length": 2.0 }))
    );
    assert!(entry.migrate(0, json!({ "length": 2.0 })).is_err());

    let dog = world.spawn();
    let post = world.spawn();
    world.insert(
        dog,
        Leash {
            to: post,
            length: 3.0,
        },
    );
    let at = Holder::Entity(dog);
    assert_eq!(
        entry.saved.to_value(&world, at).unwrap().unwrap(),
        json!({ "length": 3.0 })
    );
    let ids = |entity: Entity| (entity == post).then_some(SaveId(7));
    let (refs, dropped) = entry.saved.save_refs(&world, at, &ids).unwrap();
    assert_eq!(refs, Refs::from([("to".to_owned(), vec![SaveId(7)])]));
    assert_eq!(dropped, 0);
    assert_eq!(entry.saved.keys(&world, at), Some(vec!["to".to_owned()]));

    let pup = world.spawn();
    entry
        .saved
        .put(&mut world, Holder::Entity(pup), json!({ "length": 1.5 }))
        .unwrap();
    assert_eq!(world.get::<Leash>(pup).unwrap().to, Entity::PLACEHOLDER);
    let resolved = BTreeMap::from([("to".to_owned(), vec![post])]);
    entry
        .saved
        .load_refs(&mut world, Holder::Entity(pup), &resolved);
    assert_eq!(
        world.get::<Leash>(pup),
        Some(&Leash {
            to: post,
            length: 1.5
        })
    );
    assert!(!world.is_alive(Entity::PLACEHOLDER));
}

#[test]
#[should_panic(expected = "has a step from version 2, at or past its version 2")]
fn a_step_past_the_version_panics() {
    World::new().register_saved_with(
        "health",
        SaveOptions::<Health>::new().version(2).step(2, Ok),
    );
}
