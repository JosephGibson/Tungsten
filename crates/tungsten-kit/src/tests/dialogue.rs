use super::*;
use serde_json::{Value, json};
use tungsten_core::{Plugin, Schedule};

use crate::react::counters::CountersPlugin;
use crate::react::cues::CuesPlugin;

/// The plan's `keeper.evening`.
fn evening() -> Value {
    json!({
        "speaker": "keeper",
        "start": "greet",
        "nodes": {
            "greet": { "lines": ["Evening, warden.", { "speaker": "warden", "text": "Evening." }], "next": "offer" },
            "offer": {
                "lines": [{ "text": "The lamp by the steps is out again.", "mood": "worried" }],
                "choices": [
                    { "text": "I'll light it.", "do": [{ "flag": { "name": "lamp.promised", "target": "game" } }], "next": "thanks" },
                    { "text": "Who keeps the lamps?", "when": { "not": { "flag": { "name": "lamp.asked", "target": "game" } } },
                      "do": [{ "flag": { "name": "lamp.asked", "target": "game" } }], "next": "offer" },
                    { "text": "Not tonight.", "back": true }
                ]
            },
            "thanks": { "lines": ["Bless you."] }
        }
    })
}

fn parse(value: Value) -> Result<Conversation, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// The parse error of `value`, which must fail.
fn refused(value: Value) -> String {
    match parse(value) {
        Ok(_) => panic!("parsed"),
        Err(error) => error,
    }
}

/// `value` with `pointer`'s value set to `to`, its last key added when
/// new.
fn with(mut value: Value, pointer: &str, to: Value) -> Value {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    match value.pointer_mut(parent).unwrap() {
        Value::Object(map) => {
            map.insert(key.to_owned(), to);
        }
        Value::Array(list) => list[key.parse::<usize>().unwrap()] = to,
        other => panic!("{pointer}: {other} holds no key"),
    }
    value
}

/// A conversation of `nodes`, entered at the first.
fn chat(start: &str, nodes: Value) -> Value {
    let mut value = json!({ "speaker": "keeper", "start": start });
    value["nodes"] = nodes;
    value
}

#[test]
fn a_conversation_round_trips_its_short_and_long_lines() {
    let conversation = parse(evening()).unwrap();
    assert_eq!(
        (conversation.speaker(), conversation.start()),
        ("keeper", "greet")
    );
    assert_eq!(
        conversation.node_names().collect::<Vec<_>>(),
        ["greet", "offer", "thanks"]
    );
    let data = serde_json::to_value(&conversation).unwrap();
    assert_eq!(
        data["nodes"]["greet"]["lines"][0],
        json!("Evening, warden."),
        "short"
    );
    assert_eq!(
        data["nodes"]["greet"]["lines"][1],
        json!({ "speaker": "warden", "text": "Evening." }),
        "long"
    );
    assert_eq!(
        data["nodes"]["offer"]["choices"][2],
        json!({ "text": "Not tonight.", "back": true })
    );
    assert_eq!(parse(data).unwrap(), conversation, "round trip");

    let keeper: Speaker = serde_json::from_value(
        json!({ "name": "Old Tamsin", "portrait": "keeper_face", "pace": 30 }),
    )
    .unwrap();
    assert_eq!(keeper.portrait(None), Some("keeper_face"));
    assert_eq!(
        serde_json::to_value(&keeper).unwrap(),
        json!({ "name": "Old Tamsin", "portrait": "keeper_face", "pace": 30.0 })
    );
    let moods = Speaker::new("Tamsin")
        .with_portrait("neutral", "a")
        .with_portrait("worried", "b");
    assert_eq!(
        serde_json::to_value(&moods).unwrap()["portrait"],
        json!({ "neutral": "a", "worried": "b" })
    );
    for bad in [
        json!({ "name": "x", "pace": 0 }),
        json!({ "name": "x", "portrait": 3 }),
    ] {
        assert!(
            serde_json::from_value::<Speaker>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn an_unknown_target_or_start_names_its_node() {
    let cases = [
        (
            with(evening(), "/start", json!("hello")),
            "start: unknown node `hello`",
        ),
        (
            with(evening(), "/nodes/greet/next", json!("farewell")),
            "nodes.greet.next: unknown node `farewell`",
        ),
        (
            with(evening(), "/nodes/offer/choices/1/next", json!("lamps")),
            "nodes.offer.choices.1.next: unknown node `lamps`",
        ),
        (
            chat(
                "a",
                json!({ "a": { "lines": ["x"], "branch": [{ "when": { "flag": "f" }, "next": "b" }, { "next": "gone" }] }, "b": { "lines": ["y"] } }),
            ),
            "nodes.a.branch.1.next: unknown node `gone`",
        ),
    ];
    for (value, message) in cases {
        let error = refused(value);
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn an_unreachable_node_is_refused() {
    let mut value = evening();
    value["nodes"]["lost"] = json!({ "lines": ["Nobody hears this."] });
    let error = refused(value);
    assert!(
        error.contains("nodes.lost: unreachable from `start`"),
        "{error}"
    );
    // Reached through a choice alone is reached.
    assert!(
        parse(chat(
            "a",
            json!({ "a": { "choices": [{ "text": "on", "next": "b" }] }, "b": { "lines": ["y"] } })
        ))
        .is_ok()
    );
}

#[test]
fn a_choice_list_or_branch_without_a_way_on_is_refused() {
    let gated = with(
        evening(),
        "/nodes/offer/choices/2/when",
        json!({ "flag": "late" }),
    );
    let gated = with(
        gated,
        "/nodes/offer/choices/0/when",
        json!({ "flag": "early" }),
    );
    let error = refused(gated);
    assert!(
        error.contains("nodes.offer.choices: every choice has a `when`"),
        "{error}"
    );

    let branch = chat(
        "a",
        json!({ "a": { "lines": ["x"], "branch": [{ "next": "b" }, { "when": { "flag": "f" }, "next": "b" }] }, "b": { "lines": ["y"] } }),
    );
    let error = refused(branch);
    assert!(
        error.contains("nodes.a.branch.1.when: the last entry takes no `when`"),
        "{error}"
    );

    let two_exits = chat(
        "a",
        json!({ "a": { "lines": ["x"], "next": "b", "branch": [{ "next": "b" }] }, "b": { "lines": ["y"] } }),
    );
    assert!(refused(two_exits).contains("nodes.a: at most one of"));
    let two_backs = with(evening(), "/nodes/offer/choices/0/back", json!(true));
    assert!(
        refused(two_backs).contains("nodes.offer.choices.2.back: a node has one `back` choice")
    );
}

#[test]
fn a_cycle_of_jumps_is_refused() {
    let jumps = chat(
        "a",
        json!({ "a": { "next": "b" }, "b": { "branch": [{ "next": "a" }] } }),
    );
    let error = refused(jumps);
    assert!(
        error.contains("a cycle of nodes that may show nothing"),
        "{error}"
    );
    // A loop through nodes that show a line is a conversation's own.
    let shown = chat(
        "a",
        json!({ "a": { "lines": ["x"], "next": "b" }, "b": { "lines": ["y"], "next": "a" } }),
    );
    assert!(parse(shown).is_ok());
    // So is one through a node that offers choices.
    let offered = with(evening(), "/nodes/thanks", json!({ "next": "offer" }));
    assert!(parse(offered).is_ok());
}

#[test]
fn a_node_of_conditional_lines_alone_counts_as_a_jump() {
    let line = |text: &str| json!({ "text": text, "when": { "flag": "f" } });
    let hidden = chat(
        "a",
        json!({ "a": { "lines": [line("x")], "next": "b" }, "b": { "lines": [line("y"), line("z")], "next": "a" } }),
    );
    let error = refused(hidden);
    assert!(
        error.contains("a cycle of nodes that may show nothing"),
        "{error}"
    );
    let one_free = chat(
        "a",
        json!({ "a": { "lines": [line("x")], "next": "b" }, "b": { "lines": [line("y"), "always"], "next": "a" } }),
    );
    assert!(parse(one_free).is_ok(), "a line free of a `when` shows");
}

/// A chain of `jumps` nodes that show nothing, then one that shows a line.
fn chain_of(jumps: usize) -> Value {
    let mut nodes = serde_json::Map::new();
    for i in 0..jumps {
        let next = if i + 1 == jumps {
            "end".to_owned()
        } else {
            format!("n{}", i + 1)
        };
        nodes.insert(format!("n{i}"), json!({ "next": next }));
    }
    nodes.insert("end".into(), json!({ "lines": ["Here."] }));
    chat("n0", Value::Object(nodes))
}

#[test]
fn a_chain_of_64_jumps_is_refused_and_63_passes() {
    assert!(parse(chain_of(MAX_JUMPS)).is_ok());
    let error = refused(chain_of(MAX_JUMPS + 1));
    assert!(
        error.contains("a chain of 64 nodes that may show nothing, past 63"),
        "{error}"
    );
}

#[test]
fn the_bounds_refuse_what_passes_them() {
    // 256 nodes, each shown, chained: the most.
    let many = |count: usize| {
        let mut nodes = serde_json::Map::new();
        for i in 0..count {
            let mut node = json!({ "lines": ["."] });
            if i + 1 < count {
                node["next"] = json!(format!("n{}", i + 1));
            }
            nodes.insert(format!("n{i}"), node);
        }
        chat("n0", Value::Object(nodes))
    };
    assert!(parse(many(MAX_NODES)).is_ok());
    assert!(refused(many(MAX_NODES + 1)).contains("nodes: 257 nodes, past 256"));

    let lines = |count: usize| chat("a", json!({ "a": { "lines": vec!["."; count] } }));
    assert!(parse(lines(MAX_LINES)).is_ok());
    assert!(refused(lines(MAX_LINES + 1)).contains("nodes.a.lines: 33 lines, past 32"));

    let choices = |count: usize| {
        let list: Vec<Value> = (0..count)
            .map(|i| json!({ "text": format!("{i}") }))
            .collect();
        chat("a", json!({ "a": { "choices": list } }))
    };
    assert!(parse(choices(MAX_CHOICES)).is_ok());
    assert!(refused(choices(MAX_CHOICES + 1)).contains("nodes.a.choices: 9 choices, past 8"));

    let text = |count: usize| chat("a", json!({ "a": { "lines": ["é".repeat(count)] } }));
    assert!(parse(text(MAX_TEXT)).is_ok(), "characters, not bytes");
    assert!(
        refused(text(MAX_TEXT + 1)).contains("nodes.a.lines.0.text: 2001 characters, past 2000")
    );
    let choice_text = chat(
        "a",
        json!({ "a": { "choices": [{ "text": "x".repeat(MAX_TEXT + 1) }] } }),
    );
    assert!(refused(choice_text).contains("nodes.a.choices.0.text"));

    for (value, message) in [
        (chat("a", json!({})), "at least one node"),
        (
            chat("Bad", json!({ "Bad": { "lines": ["x"] } })),
            "nodes.Bad: not a node name",
        ),
        (
            chat(
                "a",
                json!({ "a": { "lines": [{ "text": "x", "mood": "Sad" }] } }),
            ),
            "nodes.a.lines.0.mood: not a mood name",
        ),
        (
            chat("a", json!({ "a": { "lines": [{ "txt": "x" }] } })),
            "unknown field `txt`",
        ),
    ] {
        let error = refused(value);
        assert!(error.contains(message), "{error}");
    }
}

/// A World with the dialogue kinds and the cue actions' checks, holding
/// speakers `keeper` (portraits neutral and worried) and `warden`.
fn checked_world() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    CountersPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    DialoguePlugin.build(&mut schedule, &mut world);
    let mut assets = AssetRegistry::new();
    assets.intern_sprites(["keeper_face", "keeper_worried"]);
    world.insert_resource(assets);
    let data = world.resource_mut::<DataRegistry>();
    data.insert(
        "keeper",
        Speaker::new("Old Tamsin")
            .with_portrait("neutral", "keeper_face")
            .with_portrait("worried", "keeper_worried"),
    );
    data.insert("warden", Speaker::new("Warden"));
    world
}

fn missing(world: &World, value: Value) -> (String, &'static str, String) {
    let conversation = parse(value).unwrap();
    let error = check_conversation(&conversation, world).unwrap_err();
    (error.field, error.kind, error.id)
}

#[test]
fn the_world_check_names_a_missing_speaker_mood_sprite_and_action_field() {
    let world = checked_world();
    let keeper = world
        .resource::<DataRegistry>()
        .get::<Speaker>("keeper")
        .unwrap();
    assert!(check_speaker(keeper, &world).is_ok());
    assert!(check_conversation(&parse(evening()).unwrap(), &world).is_ok());

    let owned = |field: &str, kind: &'static str, id: &str| (field.to_owned(), kind, id.to_owned());
    assert_eq!(
        missing(&world, with(evening(), "/speaker", json!("ferryman"))),
        owned("speaker", "speaker", "ferryman")
    );
    assert_eq!(
        missing(
            &world,
            with(evening(), "/nodes/greet/lines/1/speaker", json!("gull"))
        ),
        owned("nodes.greet.lines.1.speaker", "speaker", "gull")
    );
    assert_eq!(
        missing(
            &world,
            with(evening(), "/nodes/offer/lines/0/mood", json!("angry"))
        ),
        owned(
            "nodes.offer.lines.0.mood",
            "speaker's portrait mood",
            "angry"
        )
    );
    assert_eq!(
        missing(
            &world,
            with(
                evening(),
                "/nodes/offer/choices/0/do",
                json!([{ "flag": { "name": "x" } }, { "prefab": "puff" }])
            )
        ),
        owned("nodes.offer.choices.0.do.1.prefab", "prefab", "puff")
    );
    assert_eq!(
        missing(
            &world,
            with(
                evening(),
                "/nodes/thanks/lines/0",
                json!({ "text": "x", "when": { "stat": { "name": "grit" } } })
            )
        ),
        owned("nodes.thanks.lines.0.when.stat.name", "stat", "grit")
    );

    let faceless = Speaker::new("Gull").with_portrait("neutral", "gull_face");
    let error = check_speaker(&faceless, &world).unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind, error.id.as_str()),
        ("portrait.neutral", "sprite", "gull_face")
    );
}

#[test]
fn a_story_list_refuses_ability_loop_stop_and_label() {
    let world = checked_world();
    let list = |value: Value| -> Vec<CueAction> { serde_json::from_value(value).unwrap() };
    for (value, field) in [
        (
            json!([{ "flag": { "name": "a" } }, { "ability": "wand" }]),
            "do.1.ability",
        ),
        (json!([{ "loop": { "id": "hum" } }]), "do.0.loop"),
        (json!([{ "stop": { "id": "hum" } }]), "do.0.stop"),
        (json!([{ "label": { "font": "sans" } }]), "do.0.label"),
    ] {
        let error = check_list(&world, "do", &list(value.clone()), true).unwrap_err();
        assert_eq!(error.field, field, "{value}");
        assert_eq!(
            error.kind,
            "story list action (no ability, loop, stop or label)"
        );
    }
    let fine = list(json!([
        { "flag": { "name": "a", "target": "other" } },
        { "counter": { "name": "b", "add": 1, "target": "game" } },
        { "shake": 0.2 },
    ]));
    assert!(check_list(&world, "do", &fine, true).is_ok());
    let error = check_list(&world, "rewards", &fine, false).unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind),
        ("rewards.0.flag.target", "cue event with an other entity"),
        "a list with no other refuses `target: other`"
    );
    // The parse refuses a dialogue list's unknown action outright; the
    // load's error adds its path.
    let error = refused(with(evening(), "/nodes/greet/do", json!([{ "dance": {} }])));
    assert!(error.contains("unknown variant `dance`"), "{error}");
}
