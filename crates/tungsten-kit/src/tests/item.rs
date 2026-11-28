use serde_json::{Value, json};
use tungsten_core::{PluginSet, Schedule};

use super::*;
use crate::foundation::stats::Modifier;
use crate::{KitPlugins, StatsPlugin, TagsPlugin};

/// The tags the wand and its affixes name.
const TAGS: [&str; 10] = [
    "item.weapon",
    "item.wand",
    "item.one_hand",
    "item.ring",
    "skill.spell",
    "skill.attack",
    "status.chilled",
    "team.player",
    "damage.fire",
    "damage.cold",
];

/// A World with every kit plugin, the tags and `use_speed` declared, and
/// the damage types `damage.fire` and `damage.cold`.
fn rig() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    let plugins: PluginSet = KitPlugins::set()
        .without::<TagsPlugin>()
        .with(TagsPlugin::new().with_tags(TAGS))
        .without::<StatsPlugin>()
        .with(StatsPlugin::new().with_stat("use_speed", 0.0, f32::INFINITY));
    plugins.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(json!({ "damage.fire": {}, "damage.cold": {} })).unwrap();
    world.resource_mut::<DataRegistry>().insert("types", types);
    world
}

fn base(value: Value) -> Result<ItemBase, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn affix(value: Value) -> Result<Affix, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn item(value: Value) -> Result<Item, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// A tier of `level`, its weight and one range.
fn tier(level: u32, weight: f32, range: [f32; 2]) -> Value {
    json!({ "level": level, "weight": weight, "values": [range] })
}

/// One `pct` line on `stat` reading the first rolled value as a share,
/// with a `with` filter of `any`.
fn pct_line(stat: &str, any: &str) -> Value {
    json!({ "stat": stat, "modifier": { "pct": { "value": 0, "times": 0.01 } }, "with": { "any": [any] } })
}

/// §9.2's wand, its five affixes and an iron band.
fn wand_data(world: &mut World) {
    let wand = base(json!({
        "name": "Ashwood Wand",
        "icon": "wand",
        "tags": ["item", "item.weapon", "item.wand", "item.one_hand"],
        "slot": "main_hand",
        "price": 40,
        "implicit": [{ "stat": "damage", "modifier": { "pct": 0.08 }, "with": { "all": ["skill.spell"] } }],
        "weapon": { "damage": { "damage.fire": [3, 7] }, "use_time": 0.8, "crit_chance": 0.07 }
    }))
    .unwrap();
    let band =
        base(json!({ "name": "Iron Band", "icon": "band", "tags": ["item.ring"], "slot": "ring" }))
            .unwrap();
    let affixes = [
        (
            "searing",
            json!({
                "kind": "prefix", "group": "fire_damage_pct",
                "weights": [{ "tag": "item.wand", "weight": 1000 }, { "tag": "item.ring", "weight": 500 }],
                "tiers": [tier(1, 1000.0, [10.0, 16.0]), tier(20, 600.0, [17.0, 24.0]), tier(46, 300.0, [25.0, 32.0])],
                "lines": [pct_line("damage", "damage.fire")]
            }),
        ),
        (
            "spellwoven",
            json!({
                "kind": "prefix", "group": "spell_damage_pct",
                "weights": [{ "tag": "item.wand", "weight": 1000 }],
                "tiers": [tier(1, 1000.0, [10.0, 19.0]), tier(24, 600.0, [20.0, 29.0]), tier(50, 300.0, [30.0, 39.0])],
                "lines": [pct_line("damage", "skill.spell")]
            }),
        ),
        (
            "quickened",
            json!({
                "kind": "suffix", "group": "cast_speed_pct",
                "weights": [{ "tag": "item.wand", "weight": 800 }],
                "tiers": [tier(1, 1000.0, [5.0, 9.0]), tier(25, 600.0, [10.0, 14.0]), tier(55, 300.0, [15.0, 19.0])],
                "lines": [pct_line("use_speed", "skill.spell")]
            }),
        ),
        (
            "warded_fire",
            json!({
                "kind": "suffix", "group": "fire_resist",
                "weights": [{ "tag": "item", "weight": 1000 }],
                "tiers": [tier(1, 1000.0, [6.0, 11.0]), tier(18, 800.0, [12.0, 17.0]), tier(36, 500.0, [18.0, 23.0])],
                "lines": [{ "resist": "damage.fire", "modifier": { "add": { "value": 0, "times": -0.01 } } }]
            }),
        ),
        (
            "honed",
            json!({
                "kind": "prefix", "group": "local_added",
                "weights": [{ "tag": "item.wand", "weight": 0 }, { "tag": "item.weapon", "weight": 1000 }],
                "tiers": [
                    { "level": 1, "weight": 1000, "values": [[1, 2], [3, 5]] },
                    { "level": 22, "weight": 600, "values": [[3, 5], [6, 9]] },
                    { "level": 44, "weight": 300, "values": [[6, 9], [10, 14]] }
                ],
                "lines": [{ "added": { "damage.cold": [{ "value": 0 }, { "value": 1 }] }, "local": true }]
            }),
        ),
    ];
    let data = world.resource_mut::<DataRegistry>();
    data.insert("ashwood_wand", wand);
    data.insert("iron_band", band);
    for (id, value) in affixes {
        data.insert(id, affix(value).unwrap_or_else(|e| panic!("{id}: {e}")));
    }
}

/// §4.6's wand as JSON.
fn wand_json() -> Value {
    json!({
        "uid": 9137, "base": "ashwood_wand", "level": 42, "rarity": "rare", "seed": "0x9e3779b97f4a7c15",
        "affixes": [
            { "id": "searing", "tier": 2, "values": [22] },
            { "id": "quickened", "tier": 2, "values": [11] },
            { "id": "spellwoven", "tier": 2, "values": [26] },
            { "id": "warded_fire", "tier": 3, "values": [19] }
        ],
        "name": ["Ember", "Coil"]
    })
}

/// Each line as (stat or tag, modifier, its `with` filter's names).
fn lines_of(built: &BuiltItem) -> Vec<(String, Modifier, Vec<String>)> {
    built
        .lines()
        .iter()
        .map(|line| {
            if let Some((stat, line)) = line.as_stat() {
                let names = line.context_filter().names().map(str::to_owned).collect();
                (stat.to_owned(), line.modifier(), names)
            } else if let Some((tag, Some(line), _)) = line.as_resist() {
                (format!("resist {tag}"), line.modifier(), Vec::new())
            } else {
                panic!("an unexpected line {line:?}")
            }
        })
        .collect()
}

fn close(a: Modifier, b: Modifier) -> bool {
    match (a, b) {
        (Modifier::Add(x), Modifier::Add(y))
        | (Modifier::Pct(x), Modifier::Pct(y))
        | (Modifier::Mul(x), Modifier::Mul(y))
        | (Modifier::Override(x), Modifier::Override(y)) => (x - y).abs() < 1e-6,
        _ => false,
    }
}

#[test]
fn the_wand_from_its_json_round_trips() {
    let wand = item(wand_json()).unwrap();
    assert_eq!(
        (wand.uid(), wand.base(), wand.level(), wand.rarity()),
        (9137, "ashwood_wand", 42, "rare")
    );
    assert_eq!(wand.seed(), 0x9e37_79b9_7f4a_7c15);
    assert_eq!(wand.name_words().collect::<Vec<_>>(), ["Ember", "Coil"]);
    assert_eq!(
        (
            wand.affixes()[3].id(),
            wand.affixes()[3].tier(),
            wand.affixes()[3].values()
        ),
        ("warded_fire", 3, &[19.0][..])
    );
    let written = serde_json::to_value(&wand).unwrap();
    assert_eq!(written["seed"], "0x9e3779b97f4a7c15");
    assert!(written.get("count").is_none(), "a count of 1 is left out");
    assert_eq!(item(written).unwrap(), wand);
    let stack = Item::new("oil_flask").with_count(3);
    let written = serde_json::to_value(&stack).unwrap();
    assert_eq!(
        (written["count"].clone(), written["seed"].clone()),
        (json!(3), json!("0x0"))
    );
    assert_eq!(item(written).unwrap(), stack);
    for (broken, why) in [
        (json!({ "base": "ashwood_wand", "level": 0 }), "level"),
        (json!({ "base": "ashwood_wand", "seed": "9e37" }), "seed"),
        (
            json!({ "base": "ashwood_wand", "name": ["a", "b", "c", "d", "e"] }),
            "name",
        ),
        (json!({ "base": "ashwood_wand", "count": 0 }), "count"),
        (
            json!({ "unique": "cinderwake_band", "base": "iron_band" }),
            "unique",
        ),
        (
            json!({ "base": "ashwood_wand", "affixes": [{ "id": "searing", "tier": 0, "values": [1] }] }),
            "tier",
        ),
    ] {
        let error = item(broken.clone()).unwrap_err();
        assert!(error.contains(why), "{broken}: {error}");
    }
}

#[test]
fn the_wand_builds_its_affixes_lines_at_their_values() {
    let mut world = rig();
    wand_data(&mut world);
    let wand = item(wand_json()).unwrap();
    check_item(&world, "item", &wand).unwrap();
    let made = built(&world, &wand).unwrap();
    assert_eq!(
        (made.name(), made.rarity(), made.known()),
        ("Ember Coil", "rare", true)
    );
    let lines = lines_of(&made);
    let want = [
        ("damage", Modifier::Pct(0.08), vec!["skill.spell"]),
        ("damage", Modifier::Pct(0.22), vec!["damage.fire"]),
        ("use_speed", Modifier::Pct(0.11), vec!["skill.spell"]),
        ("damage", Modifier::Pct(0.26), vec!["skill.spell"]),
        ("resist damage.fire", Modifier::Add(-0.19), vec![]),
    ];
    assert_eq!(lines.len(), want.len(), "{lines:?}");
    for ((stat, modifier, with), (want_stat, want_modifier, want_with)) in lines.iter().zip(want) {
        assert_eq!(stat, want_stat);
        assert!(close(*modifier, want_modifier), "{stat}: {modifier:?}");
        assert_eq!(with, &want_with);
    }
    let block = made.block().unwrap();
    assert_eq!(
        block.damage().collect::<Vec<_>>(),
        [("damage.fire", [3.0, 7.0])]
    );
    assert_eq!((block.use_time(), block.crit_chance()), (0.8, 0.07));
    let ring = Item::new("iron_band").with_affix(RolledAffix::new("searing", 1, [12.0]));
    let ring = built(&world, &ring).unwrap();
    assert_eq!(ring.name(), "Iron Band");
    assert!(ring.block().is_none());
    assert_eq!(ring.lines()[0].text(), "+12% damage with damage.fire");
}

#[test]
fn a_local_line_folds_into_the_block() {
    let mut world = rig();
    wand_data(&mut world);
    let honed = Item::new("ashwood_wand")
        .with_level(30)
        .with_affix(RolledAffix::new("honed", 2, [4.0, 7.0]));
    check_item(&world, "item", &honed).unwrap();
    let built_wand = built(&world, &honed).unwrap();
    assert_eq!(
        built_wand.lines().len(),
        1,
        "the local line is no global line"
    );
    let block = built_wand.block().unwrap();
    assert_eq!(
        block.damage().collect::<Vec<_>>(),
        [("damage.fire", [3.0, 7.0]), ("damage.cold", [4.0, 7.0])],
        "a local `added` adds a part"
    );
    // The fold's order: the adds, then 1 + Σpct, then Πmul, by `with`.
    let keen = base(json!({
        "name": "Keen Rod", "icon": "rod",
        "implicit": [
            { "added": { "damage.fire": [2, 3] }, "local": true },
            { "stat": "damage", "modifier": { "pct": 0.5 }, "with": { "any": ["damage.fire"] }, "local": true },
            { "stat": "damage", "modifier": { "mul": 2 }, "local": true },
            { "stat": "use_time", "modifier": { "pct": -0.25 }, "local": true },
            { "stat": "crit_chance", "modifier": { "add": 2 }, "local": true }
        ],
        "weapon": { "damage": { "damage.fire": [2, 5], "damage.cold": [1, 1] }, "use_time": 1.0, "crit_chance": 0.05 }
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("keen_rod", keen);
    let block = built(&world, &Item::new("keen_rod"))
        .unwrap()
        .block()
        .cloned()
        .unwrap();
    assert_eq!(
        block.damage().collect::<Vec<_>>(),
        [("damage.cold", [2.0, 2.0]), ("damage.fire", [12.0, 24.0])],
        "fire (2+2) × 1.5 × 2 and (5+3) × 1.5 × 2; cold 1 × 2"
    );
    assert_eq!(block.use_time(), 0.75);
    assert_eq!(block.crit_chance(), 1.0, "clamped to 1");
    assert_eq!(
        block.text(),
        [
            "2–2 damage.cold",
            "12–24 damage.fire",
            "use time 0.75 s",
            "crit chance 100%"
        ]
    );
}

#[test]
fn a_local_line_without_a_block_or_with_a_holder_or_hit_scope_is_refused() {
    let local = |line: Value| {
        base(json!({ "name": "Rod", "icon": "rod", "implicit": [line],
        "weapon": { "damage": { "damage.fire": [1, 2] }, "use_time": 1 } }))
    };
    local(json!({ "stat": "damage", "modifier": { "pct": 0.1 }, "with": { "any": ["damage.fire"] }, "local": true })).unwrap();
    for (line, why) in [
        (
            json!({ "stat": "damage", "modifier": { "pct": 0.1 }, "while": { "all": ["status.chilled"] }, "local": true }),
            "takes `with` alone",
        ),
        (
            json!({ "stat": "damage", "modifier": { "pct": 0.1 }, "against": { "all": ["status.chilled"] }, "local": true }),
            "takes `with` alone",
        ),
        (
            json!({ "stat": "damage", "modifier": { "override": 3 }, "local": true }),
            "may not override",
        ),
        (
            json!({ "stat": "move_speed", "modifier": { "pct": 0.1 }, "local": true }),
            "not `move_speed`",
        ),
        (
            json!({ "stat": "use_time", "modifier": { "pct": 0.1 }, "with": { "any": ["damage.fire"] }, "local": true }),
            "folds into the block whole",
        ),
        (
            json!({ "pen": { "damage.fire": 0.1 }, "local": true }),
            "not a `pen` or `resist`",
        ),
        (
            json!({ "resist": "damage.fire", "modifier": { "add": 0.1 }, "local": true }),
            "not a `pen` or `resist`",
        ),
    ] {
        let error = local(line.clone()).unwrap_err();
        assert!(error.contains(why), "{line}: {error}");
    }
    let error = base(json!({ "name": "Charm", "icon": "charm",
        "implicit": [{ "added": { "damage.fire": [1, 2] }, "local": true }] }))
    .unwrap_err();
    assert!(
        error.contains("implicit.0: a local line folds into a `weapon` block"),
        "{error}"
    );
    // An affix's local line is refused on an instance whose base has no
    // block.
    let mut world = rig();
    wand_data(&mut world);
    let ring = Item::new("iron_band")
        .with_level(30)
        .with_affix(RolledAffix::new("honed", 2, [4.0, 7.0]));
    let missing = check_item(&world, "item", &ring).unwrap_err();
    assert_eq!(missing.field, "item.affixes.0.id");
}

#[test]
fn tiers_ascend_and_ranges_hold_or_the_affix_is_refused() {
    let with_tiers = |tiers: Value, lines: Value| {
        affix(json!({ "kind": "suffix", "group": "g", "tiers": tiers, "lines": lines }))
    };
    let one = json!([pct_line("damage", "damage.fire")]);
    with_tiers(
        json!([tier(1, 1.0, [1.0, 2.0]), tier(5, 1.0, [3.0, 4.0])]),
        one.clone(),
    )
    .unwrap();
    for (tiers, lines, why) in [
        (
            json!([tier(5, 1.0, [1.0, 2.0]), tier(5, 1.0, [3.0, 4.0])]),
            one.clone(),
            "tiers.1.level: 5 after 5",
        ),
        (
            json!([tier(9, 1.0, [1.0, 2.0]), tier(4, 1.0, [3.0, 4.0])]),
            one.clone(),
            "tiers.1.level: 4 after 9",
        ),
        (
            json!([tier(1, 1.0, [3.0, 2.0])]),
            one.clone(),
            "values.0: [3, 2]",
        ),
        (
            json!([tier(0, 1.0, [1.0, 2.0])]),
            one.clone(),
            "level: 1 to 100",
        ),
        (
            json!([tier(1, 0.0, [1.0, 2.0])]),
            one.clone(),
            "must be finite and > 0",
        ),
        (json!([]), one.clone(), "tiers: 1 to 16"),
        (
            json!([tier(1, 1.0, [1.0, 2.0])]),
            json!([]),
            "lines: 1 to 8",
        ),
        (
            json!([{ "level": 1, "weight": 1, "values": [[1, 2], [3, 4]] }, tier(9, 1.0, [3.0, 4.0])]),
            json!([{ "stat": "damage", "modifier": { "add": { "value": 1 } } }]),
            "lines.0: reads rolled value 1, past the 1 every tier rolls",
        ),
        (
            json!([tier(1, 1.0, [1.0, 2.0])]),
            json!([{ "stat": "damage", "modifier": { "add": { "value": 4 } } }]),
            "names no rolled value",
        ),
    ] {
        let error = with_tiers(tiers.clone(), lines).unwrap_err();
        assert!(error.contains(why), "{tiers}: {error}");
    }
    // The load holds the weights' tags and the lines' names.
    let world = rig();
    let stray = affix(
        json!({ "kind": "prefix", "group": "g", "weights": [{ "tag": "item.shield", "weight": 1 }],
        "tiers": [tier(1, 1.0, [1.0, 2.0])], "lines": [pct_line("damage", "damage.fire")] }),
    )
    .unwrap();
    assert_eq!(
        crate::items::affix::check_affix(&stray, &world)
            .unwrap_err()
            .field,
        "weights.0.tag"
    );
    let undeclared = affix(json!({ "kind": "prefix", "group": "g",
        "tiers": [tier(1, 1.0, [1.0, 2.0])], "lines": [pct_line("spell_echo", "damage.fire")] }))
    .unwrap();
    assert_eq!(
        crate::items::affix::check_affix(&undeclared, &world)
            .unwrap_err()
            .field,
        "lines.0.stat"
    );
}

#[test]
fn an_instance_out_of_its_tier_or_range_is_refused_with_its_field_path() {
    let mut world = rig();
    wand_data(&mut world);
    let wand = |affixes: Value| {
        item(json!({ "base": "ashwood_wand", "level": 42, "affixes": affixes })).unwrap()
    };
    for (affixes, field) in [
        (
            json!([{ "id": "searing", "tier": 3, "values": [26] }]),
            "item.affixes.0.tier",
        ),
        (
            json!([{ "id": "searing", "tier": 4, "values": [26] }]),
            "item.affixes.0.tier",
        ),
        (
            json!([{ "id": "searing", "tier": 2, "values": [25] }]),
            "item.affixes.0.values.0",
        ),
        (
            json!([{ "id": "searing", "tier": 2, "values": [20.5] }]),
            "item.affixes.0.values.0",
        ),
        (
            json!([{ "id": "searing", "tier": 2, "values": [20, 21] }]),
            "item.affixes.0.values",
        ),
        (
            json!([{ "id": "searing", "tier": 1, "values": [12] }, { "id": "searing", "tier": 2, "values": [20] }]),
            "item.affixes.1.id",
        ),
        (
            json!([{ "id": "blazing", "tier": 1, "values": [12] }]),
            "item.affixes.0.id",
        ),
    ] {
        let missing = check_item(&world, "item", &wand(affixes.clone())).unwrap_err();
        assert_eq!(missing.field, field, "{affixes}");
    }
    let lost = item(json!({ "base": "birch_wand" })).unwrap();
    assert_eq!(
        check_item(&world, "item", &lost).unwrap_err().field,
        "item.base"
    );
    let stacked = Item::new("ashwood_wand").with_count(2);
    assert_eq!(
        check_item(&world, "item", &stacked).unwrap_err().field,
        "item.count"
    );
}

#[test]
fn a_restored_item_keeps_an_unknown_base_or_affix_and_warns_once() {
    let mut world = rig();
    wand_data(&mut world);
    let mut saved = wand_json();
    saved["affixes"][1]["id"] = "hastened".into();
    saved["affixes"][2]["tier"] = 9.into();
    let wand = item(saved).unwrap();
    for _ in 0..3 {
        let made = built(&world, &wand).unwrap();
        assert!(made.known());
        assert_eq!(
            lines_of(&made).len(),
            3,
            "the implicit line, searing's and warded_fire's: the unknown affix and tier left out"
        );
    }
    assert_eq!(
        world.resource::<ItemWarnings>().len(),
        2,
        "each warned once"
    );
    let lost = Item::new("birch_wand").with_affix(RolledAffix::new("searing", 1, [12.0]));
    for _ in 0..2 {
        let made = built(&world, &lost).unwrap();
        assert_eq!((made.name(), made.known()), ("birch_wand", false));
        assert!(made.lines().is_empty() && made.block().is_none());
    }
    assert_eq!(world.resource::<ItemWarnings>().len(), 3);
    // Kept as saved: it writes back what it read.
    let written = serde_json::to_value(&wand).unwrap();
    assert_eq!(written["affixes"][1]["id"], "hastened");
    assert_eq!(written["affixes"][2]["tier"], 9);
}

#[test]
fn a_build_skips_the_lines_its_saved_values_cannot_fill() {
    let mut world = rig();
    wand_data(&mut world);
    // An affix kept in the data with a second range its saved items lack.
    let widened = affix(json!({
        "kind": "prefix", "group": "fire_damage_pct",
        "tiers": [{ "level": 1, "weight": 1000, "values": [[10, 16], [1, 3]] }],
        "lines": [pct_line("damage", "damage.fire"), { "stat": "crit_chance", "modifier": { "add": { "value": 1, "times": 0.01 } } }]
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("searing", widened);
    let saved = Item::new("iron_band").with_affix(RolledAffix::new("searing", 1, [12.0]));
    let built_ring = built(&world, &saved).unwrap();
    assert_eq!(
        built_ring.lines().len(),
        1,
        "the second line reads a value the save lacks"
    );
    assert_eq!(built_ring.lines()[0].text(), "+12% damage with damage.fire");
    // A base that lost its block: the affix's local line is left out.
    let blockless =
        base(json!({ "name": "Ashwood Wand", "icon": "wand", "slot": "main_hand" })).unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("ashwood_wand", blockless);
    let honed = Item::new("ashwood_wand")
        .with_level(30)
        .with_affix(RolledAffix::new("honed", 2, [4.0, 7.0]));
    let built_wand = built(&world, &honed).unwrap();
    assert!(built_wand.block().is_none() && built_wand.lines().is_empty());
    assert_eq!(world.resource::<ItemWarnings>().len(), 2);
}

#[test]
fn a_lines_text_is_its_own_or_reads_its_stat_number_and_scope() {
    let mut world = rig();
    let lines = json!([
        { "stat": "crit_chance", "modifier": { "add": 1 } },
        { "stat": "damage", "modifier": { "pct": 0.22 }, "with": { "any": ["damage.fire"] } },
        { "stat": "move_speed", "modifier": { "pct": -0.05 }, "while": { "all": ["status.chilled"] } },
        { "stat": "damage", "modifier": { "mul": 1.2 }, "against": { "all": ["status.chilled"] } },
        { "stat": "max_health", "modifier": { "override": 12.5 } },
        { "added": { "damage.fire": [4, 6] }, "with": { "all": ["skill.attack"] } },
        { "pen": { "damage.fire": 0.1 } },
        { "resist": "damage.fire", "modifier": { "add": -0.19 } },
        { "resist": "damage.cold", "modifier": { "add": 0.1 }, "floor": { "add": -0.05 } },
        { "stat": "damage", "modifier": { "pct": 0.3 }, "text": "Burns {0} brighter" }
    ]);
    let charm = base(json!({ "name": "Charm", "icon": "charm", "implicit": lines.as_array().unwrap()[..8].to_vec() })).unwrap();
    world.resource_mut::<DataRegistry>().insert("charm", charm);
    let texts: Vec<String> = built(&world, &Item::new("charm"))
        .unwrap()
        .lines()
        .iter()
        .map(|line| line.text().to_owned())
        .collect();
    assert_eq!(
        texts,
        [
            "+1 crit_chance",
            "+22% damage with damage.fire",
            "-5% move_speed while status.chilled",
            "×1.2 damage against status.chilled",
            "max_health = 12.5",
            "adds 4–6 damage.fire with skill.attack",
            "penetrates 10% damage.fire",
            "damage.fire resist +19%",
        ]
    );
    let more = base(json!({ "name": "Ring", "icon": "ring", "implicit": lines.as_array().unwrap()[8..].to_vec() })).unwrap();
    world.resource_mut::<DataRegistry>().insert("ring", more);
    let texts: Vec<String> = built(&world, &Item::new("ring"))
        .unwrap()
        .lines()
        .iter()
        .map(|line| line.text().to_owned())
        .collect();
    assert_eq!(
        texts,
        [
            "damage.cold resist -10%, damage.cold floor -5%",
            "Burns {0} brighter"
        ],
        "an implicit line has no rolled value for `{{0}}`"
    );
    let own = affix(json!({ "kind": "suffix", "group": "g", "tiers": [tier(1, 1.0, [10.0, 30.0])],
        "lines": [{ "stat": "damage", "modifier": { "pct": { "value": 0, "times": 0.01 } }, "text": "Burns {0}% brighter" }] }))
    .unwrap();
    world.resource_mut::<DataRegistry>().insert("bright", own);
    let ring = Item::new("ring").with_affix(RolledAffix::new("bright", 1, [17.0]));
    assert_eq!(
        built(&world, &ring).unwrap().lines()[2].text(),
        "Burns 17% brighter"
    );
}
