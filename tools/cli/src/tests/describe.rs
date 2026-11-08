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

fn with_screens() -> Value {
    let mut description = description();
    description["ui"] = json!({
        "text": ["game_title"],
        "enabled": ["saving_on"],
        "actions": [],
        "screens": ["confirm"]
    });
    description["screens"] = json!([{
        "id": "pause",
        "file": "assets/ui/screens/pause.json",
        "keys": ["resume", "save"],
        "uses": { "enabled": ["saving_on"], "sizes": ["menu_width"], "text": [] }
    }]);
    description
}

#[test]
fn describe_prints_screens() {
    let text = render(&with_screens());
    assert!(
        text.ends_with(
            "\
UI names:
  enabled (1): saving_on
  screens (1): confirm
  text (1): game_title
Screens (1):
  pause: assets/ui/screens/pause.json
    keys (2): resume, save
    enabled (1): saving_on
    sizes (1): menu_width
"
        ),
        "{text}"
    );
    let pause = named(&with_screens(), "pause");
    assert_eq!(pause["screens"].as_array().unwrap().len(), 1);
    assert!(!is_empty(&pause));
    let saving = named(&with_screens(), "saving_on");
    assert_eq!(saving["ui"]["enabled"], json!(["saving_on"]));
    assert_eq!(saving["screens"], json!([]));
    assert!(!is_empty(&saving));
    assert!(is_empty(&named(&with_screens(), "nothing")));
}

fn with_prefabs() -> Value {
    let mut description = description();
    description["assets"]["prefabs"] = json!(["goblin", "goblin_red"]);
    description["prefabs"] = json!([
        { "name": "goblin", "extends": null,
          "prefab": { "tag": "goblin", "visible": true,
                      "components": { "lifetime": { "seconds": 2.0 } } } },
        { "name": "goblin_red", "extends": "goblin",
          "prefab": { "sprite": { "asset_id": "red", "color": [255, 255, 255, 255], "z_order": 0 },
                      "tag": "goblin", "visible": true,
                      "components": { "lifetime": { "seconds": 4.0 }, "player": null } } }
    ]);
    description
}

#[test]
fn describe_prints_a_prefabs_resolved_fields() {
    let kept = named(&with_prefabs(), "goblin_red");
    assert!(!is_empty(&kept));
    assert_eq!(
        render(&kept),
        "\
Assets:
  prefabs (1): goblin_red
Prefabs (1):
  goblin_red (extends goblin)
    components.lifetime: {\"seconds\":4.0}
    components.player: null
    sprite: {\"asset_id\":\"red\",\"color\":[255,255,255,255],\"z_order\":0}
    tag: \"goblin\"
    visible: true
"
    );
    let text = render(&with_prefabs());
    assert!(
        text.contains("Prefabs (2):\n  goblin\n    components.lifetime: {\"seconds\":2.0}\n"),
        "{text}"
    );
    assert!(is_empty(&named(&with_prefabs(), "nothing")));
}
