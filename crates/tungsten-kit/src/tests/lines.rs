use super::*;
use serde_json::json;

use crate::Stats;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_string()).collect()
}

fn scoped(modifier: Modifier, filter: TagFilter) -> Line {
    Line::new(modifier).with(filter)
}

#[test]
fn a_with_line_reaches_only_a_read_whose_context_passes_and_never_get() {
    let mut stats = Stats::new().with(Stats::DAMAGE, 10.0);
    let spells = TagFilter::new().all(["skill.spell"]);
    stats.add_line(
        Stats::DAMAGE,
        "passive:spellcraft",
        scoped(Modifier::Pct(0.4), spells),
    );
    assert_eq!(stats.get(Stats::DAMAGE), Some(10.0), "never `get`");
    let spell = names(&["skill.projectile", "skill.spell"]);
    let melee = names(&["skill.melee"]);
    assert_eq!(
        stats.read(Stats::DAMAGE, &Read::new(&[TagSet::Names(&spell)])),
        Some(14.0)
    );
    assert_eq!(
        stats.read(Stats::DAMAGE, &Read::new(&[TagSet::Names(&melee)])),
        Some(10.0)
    );
    assert_eq!(stats.read(Stats::DAMAGE, &Read::default()), Some(10.0));

    // Several sets match together without merging, each name with its
    // dotted ancestors.
    let fire_spells = TagFilter::new().all(["skill.spell", "damage"]);
    stats.add_line(
        Stats::DAMAGE,
        "item:9137",
        scoped(Modifier::Add(2.0), fire_spells),
    );
    let fire = [TagSet::Names(&spell), TagSet::Name("damage.fire")];
    let read = stats.read(Stats::DAMAGE, &Read::new(&fire)).unwrap();
    assert!((read - 16.8).abs() < 1e-5, "(10 + 2) × 1.4: {read}");
    let tags = Tags::new(["damage.cold"]);
    let cold = [TagSet::Names(&spell), TagSet::Tags(&tags)];
    let read = stats.read(Stats::DAMAGE, &Read::new(&cold)).unwrap();
    assert!(
        (read - 16.8).abs() < 1e-5,
        "a cold part is `damage` too: {read}"
    );
    assert_eq!(
        stats.read(Stats::DAMAGE, &Read::new(&[TagSet::Name("damage.fire")])),
        Some(10.0),
        "`all` wants both"
    );
    assert_eq!(stats.get(Stats::DAMAGE), Some(10.0));
    assert_eq!(stats.read("luck", &Read::new(&fire)), None, "no such stat");
}

#[test]
fn an_against_line_reads_the_other_party_at_a_hit_only() {
    let mut stats = Stats::new().with(Stats::DAMAGE, 10.0);
    let chilled = TagFilter::new().all(["status.chilled"]);
    stats.add_line(
        Stats::DAMAGE,
        "passive:cold_snap",
        Line::new(Modifier::Pct(0.2)).against(chilled),
    );
    let brute = Tags::new(["status.chilled", "team.enemy"]);
    let warm = Tags::new(["team.enemy"]);
    let hit = Read::default();
    assert_eq!(
        stats.read(Stats::DAMAGE, &hit.against(Some(&brute))),
        Some(12.0)
    );
    assert_eq!(
        stats.read(Stats::DAMAGE, &hit.against(Some(&warm))),
        Some(10.0)
    );
    assert_eq!(stats.read(Stats::DAMAGE, &hit.against(None)), Some(10.0));
    assert_eq!(stats.read(Stats::DAMAGE, &hit), Some(10.0), "no hit");

    // A filter of only `none` passes a party without the tag at a hit,
    // and still needs the hit.
    let not_boss = TagFilter::new().none(["boss"]);
    stats.add_line(
        Stats::DAMAGE,
        "passive:bully",
        Line::new(Modifier::Mul(2.0)).against(not_boss),
    );
    assert_eq!(stats.read(Stats::DAMAGE, &hit.against(None)), Some(20.0));
    assert_eq!(stats.read(Stats::DAMAGE, &hit), Some(10.0));
    assert_eq!(stats.get(Stats::DAMAGE), Some(10.0));
}

#[test]
fn a_while_line_reads_the_holders_tags_in_a_context_read_and_never_in_get() {
    let mut stats = Stats::new().with(Stats::CRIT_CHANCE, 0.1);
    let focused = TagFilter::new().all(["status.focused"]);
    stats.add_line(
        Stats::CRIT_CHANCE,
        "passive:precision",
        Line::new(Modifier::Pct(0.5)).while_holder(focused),
    );
    let focus = Tags::new(["status.focused"]);
    let calm = Tags::new(["team.player"]);
    let read = Read::default();
    let value = stats
        .read(Stats::CRIT_CHANCE, &read.holder(Some(&focus)))
        .unwrap();
    assert!((value - 0.15).abs() < 1e-6, "{value}");
    assert_eq!(
        stats.read(Stats::CRIT_CHANCE, &read.holder(Some(&calm))),
        Some(0.1)
    );
    assert_eq!(stats.read(Stats::CRIT_CHANCE, &read), Some(0.1), "no tags");
    assert_eq!(stats.get(Stats::CRIT_CHANCE), Some(0.1), "never `get`");
    // The holder's tags are not the context.
    let context = [TagSet::Tags(&focus)];
    assert_eq!(
        stats.read(Stats::CRIT_CHANCE, &Read::new(&context)),
        Some(0.1)
    );
}

#[test]
#[should_panic(expected = "a scoped line may not override")]
fn a_scoped_override_is_refused_by_add_line() {
    let mut stats = Stats::new().with(Stats::DAMAGE, 10.0);
    stats.add_line(
        Stats::DAMAGE,
        "curse",
        Line::new(Modifier::Override(0.0)).with(TagFilter::new().all(["skill.spell"])),
    );
}

#[test]
fn a_scoped_override_is_refused_at_load_and_an_unscoped_one_kept() {
    let line = json!({ "modifier": { "override": 0 }, "against": { "all": ["boss"] } });
    let error = serde_json::from_value::<Line>(line).unwrap_err();
    assert!(error.to_string().contains("may not override"), "{error}");
    let saved = json!({ "damage": { "base": 1, "lines": [["curse", { "modifier": { "override": 0 }, "while": { "all": ["cursed"] } }]] } });
    assert!(serde_json::from_value::<Stats>(saved).is_err());
    let unscoped: Line = serde_json::from_value(json!({ "modifier": { "override": 5 } })).unwrap();
    assert!(!unscoped.is_scoped());
    let mut stats = Stats::new().with(Stats::DAMAGE, 10.0);
    stats.add_line(Stats::DAMAGE, "stone", unscoped);
    assert_eq!(
        stats.get(Stats::DAMAGE),
        Some(5.0),
        "an unscoped override is `add_modifier`'s"
    );
}

#[test]
fn the_data_form_round_trips_and_leaves_out_filters_that_pass_everything() {
    let line = Line::new(Modifier::Mul(0.5))
        .with(TagFilter::new().all(["hit.blast"]))
        .while_holder(TagFilter::new().none(["status.exposed"]));
    let data = serde_json::to_value(&line).unwrap();
    assert_eq!(
        data,
        json!({ "modifier": { "mul": 0.5 }, "with": { "all": ["hit.blast"] }, "while": { "none": ["status.exposed"] } })
    );
    assert_eq!(serde_json::from_value::<Line>(data).unwrap(), line);
    let bare = serde_json::to_value(Line::new(Modifier::Add(1.0))).unwrap();
    assert_eq!(bare, json!({ "modifier": { "add": 1.0 } }));
    for bad in [
        json!({ "modifier": { "add": 1 }, "against": { "all": ["Boss"] } }),
        json!({ "modifier": { "add": 1 }, "spare": 0 }),
        json!({ "with": { "all": ["boss"] } }),
    ] {
        assert!(
            serde_json::from_value::<Line>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_read_at_use_leaves_against_and_state_lines_for_the_hit() {
    let mut stats = Stats::new();
    let chained = TagFilter::new().all(["delivery.chained"]);
    let chilled = TagFilter::new().all(["status.chilled"]);
    let spells = TagFilter::new().all(["skill.spell"]);
    stats.add_line(
        Stats::DAMAGE,
        "support:ricochet",
        Line::new(Modifier::Mul(0.85)).with(chained),
    );
    stats.add_line(
        Stats::DAMAGE,
        "passive:cold_snap",
        Line::new(Modifier::Pct(0.2)).against(chilled),
    );
    stats.add_line(
        Stats::DAMAGE,
        "passive:spellcraft",
        Line::new(Modifier::Pct(0.4)).with(spells),
    );
    let lines: Vec<(&str, &str)> = stats
        .lines()
        .map(|(stat, source, _)| (stat, source))
        .collect();
    assert_eq!(
        lines.len(),
        3,
        "a stat the entity lacks keeps its lines: {lines:?}"
    );
    let skill = names(&["skill.spell"]);
    let context = [TagSet::Names(&skill)];
    let mut sums = stats.sums(Stats::DAMAGE, &Read::new(&context).at_use());
    assert_eq!((sums.add(), sums.mul()), (0.0, 1.0), "no base");
    assert!((sums.pct() - 0.4).abs() < 1e-6);
    let deferred: Vec<&str> = sums.deferred().map(|(source, _)| source).collect();
    assert_eq!(
        deferred,
        ["support:ricochet", "passive:cold_snap"],
        "in add order"
    );
    assert!(
        stats
            .lines()
            .all(|(_, source, line)| line.waits_for_hit() == (source != "passive:spellcraft"))
    );

    // The hit folds them in where they pass, finishing the use's sums.
    let brute = Tags::new(["status.chilled"]);
    let at_hit = [TagSet::Names(&skill), TagSet::Name("delivery.chained")];
    let hit = Read::new(&at_hit).against(Some(&brute));
    let lines: Vec<Line> = sums.deferred().map(|(_, line)| line.clone()).collect();
    for line in &lines {
        if hit.reach(line) == Reach::Applies {
            sums.fold(line.modifier());
        }
    }
    sums.fold(Modifier::Add(10.0));
    let value = sums.value(0.0, f32::INFINITY);
    assert!((value - 10.0 * 1.6 * 0.85).abs() < 1e-4, "{value}");

    // A plain read takes what passes and defers nothing.
    let plain = stats.sums(Stats::DAMAGE, &Read::new(&context));
    assert_eq!(plain.deferred().count(), 0);
}

#[test]
fn a_skipped_line_names_its_filter() {
    let line = Line::new(Modifier::Add(1.0))
        .with(TagFilter::new().all(["skill.melee"]))
        .while_holder(TagFilter::new().all(["status.focused"]))
        .against(TagFilter::new().all(["boss"]));
    let focus = Tags::new(["status.focused"]);
    let boss = Tags::new(["boss"]);
    let melee = names(&["skill.melee"]);
    let context = [TagSet::Names(&melee)];
    assert_eq!(Read::default().reach(&line), Reach::Skipped(Skip::Context));
    let read = Read::new(&context);
    assert_eq!(read.reach(&line), Reach::Skipped(Skip::Holder));
    let read = read.holder(Some(&focus));
    assert_eq!(read.reach(&line), Reach::Skipped(Skip::NoHit));
    assert_eq!(
        read.against(None).reach(&line),
        Reach::Skipped(Skip::Against)
    );
    assert_eq!(read.against(Some(&boss)).reach(&line), Reach::Applies);
    assert_eq!(read.at_use().reach(&line), Reach::Deferred);
}

#[test]
fn sums_finish_the_formula_as_get_does() {
    let mut stats = Stats::new().with("power", 7.0);
    let mut sums = Sums::new(7.0);
    for modifier in [
        Modifier::Add(0.1),
        Modifier::Pct(0.3),
        Modifier::Mul(1.1),
        Modifier::Pct(-0.05),
        Modifier::Add(2.2),
        Modifier::Mul(0.9),
    ] {
        stats.add_modifier("power", "x", modifier);
        sums.fold(modifier);
    }
    let value = sums.value(f32::NEG_INFINITY, f32::INFINITY);
    assert_eq!(Some(value), stats.get("power"));
    assert_eq!(Some(value), stats.read("power", &Read::default()));
    sums.fold(Modifier::Override(3.0));
    assert_eq!(sums.override_value(), Some(3.0));
    assert_eq!(sums.value(0.0, 2.0), 2.0, "clamped");
    let mut zero = Sums::new(5.0);
    zero.fold(Modifier::Mul(0.0));
    zero.fold(Modifier::Mul(1e30));
    assert_eq!((zero.mul(), zero.value(f32::MIN, f32::MAX)), (0.0, 0.0));
}
