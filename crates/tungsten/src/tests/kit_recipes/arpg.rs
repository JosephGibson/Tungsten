//! The arpg fixture, the ARPG design's genre (`D-218`): §9.1's caster in
//! data, Cinder Volley with its three supports' and a ring's skill-local
//! lines, on a holder whose stats carry the passives', the wand's and the
//! status's lines, beside a chilled fen brute. The fixture's damage types
//! are `elemental.fire` and `elemental.cold`, names the other fixtures
//! declare, so coverage's set holds. 7.10g's adept casts cinder in the aim
//! pose nearest its aim's elevation, and cinder names its icon. 8.1's
//! second plan: §9.2's ashwood wand and iron band with the wand's five
//! affixes, `honed` local; the exile, who wears §4.6's wand, its lines on
//! the exile, and keeps it through a save (§16's check); a rare wand lying
//! labelled, picked up whole.

use tungsten_core::Entity;

use super::{DTS, recipe, spawn};
use crate::kit::{
    Affix, Damaged, DamagedPart, Inventory, Item, ItemBase, ItemPlace, KitPlugins, KitRng,
    Modifier, Read, Resist, Stats, TagSet, Tags, built, give_item, move_item,
};
use crate::testing::Harness;

const GENRE: &str = "arpg";

/// §9.1's pinned seeds, as the kit's sheet test pins them: the first draw
/// within 10⁻⁴ of 0.5 at both, the second missing a 0.09 crit chance at the
/// first and making it at the second.
const PLAIN: u64 = 7228;
const CRIT: u64 = 2638;

/// The volley's hit on the brute, its parts' amounts, and the caster, after
/// `attack` is pressed under `seed` with the brute in the caster's hitbox.
fn volley(dt: f32, seed: u64) -> (Harness, Entity, Damaged, Vec<f32>) {
    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    let caster = spawn(&mut harness, "caster", [0.0, 0.0]);
    let brute = spawn(&mut harness, "brute", [40.0, 0.0]);
    harness.step(1);
    harness.world_mut().insert_resource(KitRng::seeded(seed));
    harness.press_action("attack");
    let mut hits = Vec::new();
    let mut parts = Vec::new();
    for _ in 0..4 {
        harness.step(1);
        hits.extend(
            harness
                .events::<Damaged>()
                .filter(|hit| hit.entity == brute)
                .copied(),
        );
        parts.extend(
            harness
                .events::<DamagedPart>()
                .filter(|part| part.entity == brute)
                .map(|part| part.amount),
        );
    }
    assert_eq!(hits.len(), 1, "dt {dt}, seed {seed}: one hit");
    (harness, caster, hits[0], parts)
}

#[test]
fn nine_ones_volley_hits_32_07_and_crits_60_94_from_data() {
    for dt in DTS {
        for (seed, crit, want) in [(PLAIN, false, 32.07), (CRIT, true, 60.94)] {
            let (_, _, hit, mut parts) = volley(dt, seed);
            assert!(
                (hit.amount - want).abs() < 0.005 && hit.crit == crit,
                "dt {dt}, seed {seed}: {} (crit {})",
                hit.amount,
                hit.crit
            );
            // Half the fire to cold: 10 of fire at fire's 0.6 and the
            // ring's 0.10, 10 of cold at cold's 0.75, a crit's 1.9 on both.
            let scale = if crit { 1.9 } else { 1.0 };
            parts.sort_by(f32::total_cmp);
            assert!(
                parts.len() == 2
                    && (parts[0] - 14.8456 * scale).abs() < 0.01
                    && (parts[1] - 17.226 * scale).abs() < 0.01,
                "dt {dt}, seed {seed}: {parts:?}"
            );
        }
    }
}

#[test]
fn nine_ones_volley_spends_its_sheets_cost() {
    for dt in DTS {
        let (harness, caster, ..) = volley(dt, PLAIN);
        let mana = harness
            .world()
            .get::<Stats>(caster)
            .unwrap()
            .base("mana")
            .unwrap();
        assert!(
            (mana - (100.0 - 20.592)).abs() < 0.005,
            "dt {dt}: 12 × 1.3 × 1.2 × 1.1 off 100: {mana}"
        );
    }
}

/// 7.10g: the adept, aimed by its game at the nearest enemy, casts cinder
/// in the aim pose nearest the brute's elevation, flipped on the left, for
/// the pose's 200 ms; cinder names its icon for a bar.
#[test]
fn the_caster_casts_cinder_in_its_aim_pose() {
    use glam::Vec2;
    use tungsten_core::components::SpriteFlip;
    use tungsten_core::{AnimationState, DataRegistry, Transform};

    use super::frames_for;
    use crate::kit::{Abilities, Aim, TagFilter, Target};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let adept = spawn(&mut harness, "adept", [0.0, 0.0]);
        let brute = spawn(&mut harness, "brute", [24.0, -32.0]);
        let enemy = Target::nearest(TagFilter::new().all(["team.enemy"]));
        harness.world_mut().insert(adept, Aim::at(enemy));
        harness.step(1);
        let shown = |harness: &Harness| {
            let world = harness.world();
            let flipped = world.get::<SpriteFlip>(adept).is_some_and(|flip| flip.0);
            let state = world.get::<AnimationState>(adept).unwrap();
            (state.animation_id.clone(), flipped)
        };
        {
            let world = harness.world();
            let abilities = world.get::<Abilities>(adept).unwrap();
            let data = world.resource::<DataRegistry>();
            assert_eq!(abilities.icon(data, "cinder"), Some("cinder_icon"));
        }
        assert_eq!(shown(&harness), ("adept_idle".into(), false), "dt {dt}");
        let cast = |harness: &mut Harness| {
            harness.press_action("attack");
            harness.step(1);
            harness.release_action("attack");
        };
        // 53 degrees up and ahead: the up-forward pose of five.
        cast(&mut harness);
        assert_eq!(
            shown(&harness),
            ("adept_cast_up_fwd".into(), false),
            "dt {dt}"
        );
        for _ in 1..frames_for(0.2, dt) {
            harness.step(1);
            assert_eq!(shown(&harness).0, "adept_cast_up_fwd", "dt {dt}");
        }
        harness.step(1);
        assert_eq!(
            shown(&harness).0,
            "adept_idle",
            "dt {dt}: its pose's 200 ms"
        );
        // Level and behind: the forward pose, flipped.
        harness
            .world_mut()
            .get_mut::<Transform>(brute)
            .unwrap()
            .position = Vec2::new(-40.0, 0.0);
        harness.step(1);
        cast(&mut harness);
        assert_eq!(shown(&harness), ("adept_cast_fwd".into(), true), "dt {dt}");
    }
}

#[test]
fn the_wand_and_its_affixes_load() {
    let harness = recipe(GENRE, KitPlugins::set(), DTS[0]);
    let world = harness.world();
    let data = world.resource::<tungsten_core::DataRegistry>();
    let wand = data.get::<ItemBase>("ashwood_wand").unwrap();
    assert_eq!(
        (wand.name(), wand.slot()),
        ("Ashwood Wand", Some("main_hand"))
    );
    let block = wand.weapon().unwrap();
    assert_eq!(
        block.damage().collect::<Vec<_>>(),
        [("elemental.fire", [3.0, 7.0])]
    );
    assert_eq!(
        data.get::<ItemBase>("iron_band").unwrap().slot(),
        Some("ring")
    );
    let mut affixes: Vec<&str> = data.ids("affixes").collect();
    affixes.sort_unstable();
    assert_eq!(
        affixes,
        ["honed", "quickened", "searing", "spellwoven", "warded_fire"]
    );
    assert_eq!(
        data.get::<Affix>("searing").unwrap().tiers()[1].values(),
        [[17.0, 24.0]]
    );
    // §4.6's wand, its affixes on the fixture's damage types.
    let rare = rare_wand();
    let made = built(world, &rare).unwrap();
    assert_eq!(made.name(), "Ember Coil");
    let texts: Vec<&str> = made
        .lines()
        .iter()
        .map(crate::kit::BuiltLine::text)
        .collect();
    assert_eq!(
        texts,
        [
            "+8% damage with skill.spell",
            "+22% damage with elemental.fire",
            "+11% use_speed with skill.spell",
            "+26% damage with skill.spell",
            "+19% fire resistance",
        ]
    );
    let (tag, line, _) = made.lines()[4].as_resist().unwrap();
    assert_eq!(tag, "elemental.fire");
    assert!(matches!(line.unwrap().modifier(), Modifier::Add(v) if (v + 0.19).abs() < 1e-6));
    // `honed` folds into the block: a cold part beside the fire.
    let honed = Item::new("ashwood_wand")
        .with_level(30)
        .with_affix(crate::kit::RolledAffix::new("honed", 2, [4.0, 7.0]));
    let made = built(world, &honed).unwrap();
    assert_eq!(
        made.block().unwrap().damage().collect::<Vec<_>>(),
        [
            ("elemental.fire", [3.0, 7.0]),
            ("elemental.cold", [4.0, 7.0])
        ]
    );
}

/// §4.6's wand as JSON, its affixes on the fixture's damage types.
fn rare_wand() -> Item {
    serde_json::from_value(serde_json::json!({
        "uid": 9137, "base": "ashwood_wand", "level": 42, "rarity": "rare", "seed": "0x9e3779b97f4a7c15",
        "affixes": [
            { "id": "searing", "tier": 2, "values": [22] },
            { "id": "quickened", "tier": 2, "values": [11] },
            { "id": "spellwoven", "tier": 2, "values": [26] },
            { "id": "warded_fire", "tier": 3, "values": [19] }
        ],
        "name": ["Ember", "Coil"]
    }))
    .unwrap()
}

/// The exile's reads with the wand worn: damage in a spell's context and
/// in a fire part's, use speed in a spell's, and its fire factor.
fn wand_reads(harness: &Harness, exile: Entity) -> [f32; 4] {
    let stats = harness.world().get::<Stats>(exile).unwrap();
    let spell = [TagSet::Name("skill.spell")];
    let fire = [TagSet::Name("elemental.fire")];
    let factor = harness
        .world()
        .get::<Resist>(exile)
        .unwrap()
        .factor(&Tags::new(["elemental.fire"]));
    [
        stats.read(Stats::DAMAGE, &Read::new(&spell)).unwrap(),
        stats.read(Stats::DAMAGE, &Read::new(&fire)).unwrap(),
        stats.read("use_speed", &Read::new(&spell)).unwrap(),
        factor,
    ]
}

/// §16's check: an item from §4.6's JSON equips, shows its lines and saves
/// round trip.
#[test]
fn the_wand_from_its_json_equips_shows_its_lines_and_saves_round_trip() {
    use tungsten_core::save::{self, capture_world, restore_world};

    #[derive(Debug, Clone, Copy)]
    struct Restored;

    let level = |dt: f32| {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let exile = spawn(&mut harness, "exile", [0.0, 0.0]);
        save::name(harness.world_mut(), exile, None, "exile").unwrap();
        (harness, exile)
    };
    let close =
        |got: [f32; 4], want: [f32; 4]| got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-4);
    // 10 × (1 + 0.08 + 0.26) with spells, 10 × 1.22 with fire, 1 × 1.11,
    // and fire's factor 1 − 0.19.
    let worn = [13.4, 12.2, 1.11, 0.81];
    for dt in DTS {
        let (mut kept, exile) = level(dt);
        assert!(
            close(wand_reads(&kept, exile), [10.0, 10.0, 1.0, 1.0]),
            "dt {dt}: bare"
        );
        give_item(kept.world_mut(), exile, rare_wand()).unwrap();
        move_item(
            kept.world_mut(),
            exile,
            ItemPlace::Cell(0),
            ItemPlace::worn("main_hand"),
        )
        .unwrap();
        kept.step(2);
        assert!(
            close(wand_reads(&kept, exile), worn),
            "dt {dt}: {:?}",
            wand_reads(&kept, exile)
        );
        let shown = |harness: &Harness, exile: Entity| -> (String, Vec<String>) {
            let item = harness
                .world()
                .get::<Inventory>(exile)
                .unwrap()
                .worn("main_hand")
                .unwrap()
                .clone();
            let made = built(harness.world(), &item).unwrap();
            (
                made.name().to_owned(),
                made.lines()
                    .iter()
                    .map(|line| line.text().to_owned())
                    .collect(),
            )
        };
        let (name, lines) = shown(&kept, exile);
        assert_eq!(name, "Ember Coil");
        assert_eq!(lines.len(), 5, "dt {dt}: {lines:?}");
        let saved = capture_world(kept.world()).unwrap();
        let (mut loaded, _) = level(dt);
        restore_world(loaded.world_mut(), &saved, &(Restored,)).unwrap();
        let back: Vec<Entity> = loaded
            .world()
            .query::<(Entity, &Inventory)>()
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(back.len(), 1, "dt {dt}: the exile came back");
        let restored = back[0];
        assert_eq!(
            loaded
                .world()
                .get::<Inventory>(restored)
                .unwrap()
                .worn("main_hand"),
            kept.world()
                .get::<Inventory>(exile)
                .unwrap()
                .worn("main_hand"),
            "dt {dt}: the wand as it was saved"
        );
        assert!(
            close(wand_reads(&loaded, restored), worn),
            "dt {dt}: its lines on the exile again"
        );
        assert_eq!(
            shown(&loaded, restored),
            (name, lines),
            "dt {dt}: shown the same"
        );
    }
}

#[test]
fn a_rare_wand_on_the_ground_is_labelled_and_picked_up() {
    use crate::kit::{GroundItem, HitShape, Hurtbox};
    use tungsten_core::physics::{Position, PrevPosition};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let world = harness.world_mut();
        world.insert_resource(crate::WindowSize {
            width: 800,
            height: 600,
        });
        world.insert_resource(tungsten_core::ScaleFactor::default());
        let exile = spawn(&mut harness, "exile", [0.0, 0.0]);
        let wand = spawn(&mut harness, "wand_drop", [120.0, 40.0]);
        harness.step(1);
        let labels: Vec<String> = harness
            .draw()
            .expect("a frame ran")
            .world_labels
            .iter()
            .map(|run| run.content.clone())
            .collect();
        assert_eq!(labels, ["Ember Coil"], "dt {dt}: labelled by its own name");
        assert!(
            harness.world().get::<GroundItem>(wand).is_some(),
            "dt {dt}: no collector near"
        );
        // The exile collects with a hurtbox, walked onto the wand.
        harness
            .world_mut()
            .insert(exile, Hurtbox::new(HitShape::circle(8.0)));
        let at = harness.world().get::<Position>(wand).unwrap().0;
        harness.world_mut().insert(exile, Position(at));
        harness.world_mut().insert(exile, PrevPosition(at));
        harness.step(2);
        let bag = harness.world().get::<Inventory>(exile).unwrap();
        let picked = bag.cell(0).expect("in the bag");
        assert_eq!(
            (picked.base(), picked.rarity(), picked.affixes().len()),
            ("ashwood_wand", "rare", 1),
            "dt {dt}: the rolled wand whole"
        );
        assert!(
            !harness.world().is_alive(wand),
            "dt {dt}: gone from the ground"
        );
    }
}
