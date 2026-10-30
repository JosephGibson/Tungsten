use super::*;

fn description() -> Value {
    json!({
        "version": 1,
        "components": [
            { "name": "lifetime", "summary": "Despawns its entity.", "one_of": false,
              "fields": [{ "name": "seconds", "type": "f32 (> 0)", "doc": "how long", "default": "1.0" }],
              "events": [], "genres": [], "sample": "{}" },
            { "name": "rigid_body" }
        ],
        "shapes": [],
        "kinds": [{ "name": "enemy", "summary": "An enemy's numbers." }],
        "script": {
            "actions": [{ "name": "start_run", "doc": "Starts a run." }],
            "probes": [{ "name": "entities", "doc": "How many entities are alive." }]
        },
        "assets": {
            "sprites": ["dot", "start_run"],
            "sounds": [],
            "data": { "enemy": ["goblin", "troll"] }
        },
        "errors": []
    })
}

#[test]
fn the_text_lists_each_section_that_has_entries() {
    assert_eq!(
        render(&description()),
        "\
Components (2):
  lifetime: Despawns its entity.
    seconds: f32 (> 0) = 1.0 \u{2014} how long
  rigid_body
Data kinds (1):
  enemy: An enemy's numbers.
Script actions (1):
  start_run: Starts a run.
Script probes (1):
  entities: How many entities are alive.
Assets:
  sprites (2): dot, start_run
  data enemy (2): goblin, troll
"
    );
}

#[test]
fn a_name_keeps_the_entries_so_named() {
    let kept = named(&description(), "start_run");
    assert_eq!(
        render(&kept),
        "\
Script actions (1):
  start_run: Starts a run.
Assets:
  sprites (1): start_run
"
    );
    let kind = named(&description(), "enemy");
    assert_eq!(kind["kinds"].as_array().unwrap().len(), 1);
    assert_eq!(kind["assets"]["data"]["enemy"], json!(["goblin", "troll"]));
    assert!(!is_empty(&kind));
    assert!(is_empty(&named(&description(), "nothing")));
    assert_eq!(
        named(&description(), "troll")["assets"]["data"],
        json!({ "enemy": ["troll"] })
    );
}
