use serde_json::json;
use tungsten_core::physics::{PhysicsPlugin, Position, PrevPosition};
use tungsten_core::save::{self as saves, capture_world, restore_world};
use tungsten_core::text::TextShadow;
use tungsten_core::ui::{NamedStyle, TextOverrides};
use tungsten_core::{
    CommandBuffer, EventQueue, PhysicsConfig, Plugin, PrefabRegistry, Stage, TilemapRegistry, Time,
    Transform,
};

use super::*;
use crate::foundation::trace::KitTrace;
use crate::items::inventory::{give, inventory};
use crate::{HitShape, Hurtbox, KitPlugins, PickedUp, Pickup};

/// What a restore's spawns take.
#[derive(Debug, Clone, Copy)]
struct Restored;

const STEP: f32 = 1.0 / 60.0;

/// A World with physics, the kit and labels, the flask base (stacks of 5,
/// dropped onto `flask_drop`), a rare wand base and their prefabs.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    world.insert_resource(TilemapRegistry::new());
    world.insert_resource(PhysicsConfig::default());
    world.insert_resource(WorldLabels::default());
    let mut prefabs = PrefabRegistry::new();
    for (id, prefab) in [
        (
            "flask_drop",
            json!({ "components": {
            "pickup": { "shape": { "circle": 4 } },
            "ground_item": { "item": "oil_flask", "offset": [0, -12] }
        } }),
        ),
        (
            "wand_drop",
            json!({ "components": {
            "pickup": { "shape": { "circle": 4 } },
            "ground_item": { "item": { "base": "wand", "rarity": "rare" } }
        } }),
        ),
    ] {
        prefabs.insert(id.to_owned(), serde_json::from_value(prefab).unwrap());
    }
    world.insert_resource(prefabs);
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    KitPlugins::set().build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    for (id, base) in [
        (
            "oil_flask",
            json!({ "name": "Oil Flask", "icon": "flask", "stack": 5, "ground": "flask_drop" }),
        ),
        (
            "wand",
            json!({ "name": "Wand", "icon": "wand", "slot": "hand", "ground": "wand_drop" }),
        ),
        ("stone", json!({ "name": "Stone", "icon": "stone" })),
    ] {
        let base: ItemBase = serde_json::from_value(base).unwrap();
        world.resource_mut::<DataRegistry>().insert(id, base);
    }
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(STEP);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

/// A collector at `x` with a bag of `cells`.
fn collector(world: &mut World, x: f32, cells: u32) -> Entity {
    world.spawn_with((
        Transform::from_position(Vec2::new(x, 0.0)),
        Position(Vec2::new(x, 0.0)),
        Hurtbox::new(HitShape::circle(6.0)),
        Inventory::new(cells),
    ))
}

/// `item` lying on a pickup at `x`.
fn lying(world: &mut World, x: f32, item: Item) -> Entity {
    world.spawn_with((
        Position(Vec2::new(x, 0.0)),
        PrevPosition(Vec2::new(x, 0.0)),
        Pickup::new(HitShape::circle(4.0)),
        GroundItem::new(item),
    ))
}

fn held(world: &World, holder: Entity, base: &str) -> u32 {
    inventory(world, holder).map_or(0, |bag| bag.count_of(base))
}

fn picked(world: &World) -> usize {
    world.resource::<EventQueue<PickedUp>>().iter().count()
}

#[test]
fn a_ground_item_goes_into_the_collectors_bag() {
    let (mut world, mut schedule) = rig();
    let warden = collector(&mut world, 0.0, 4);
    let flasks = lying(&mut world, 2.0, Item::new("oil_flask").with_count(3));
    step(&mut world, &mut schedule);
    assert_eq!(held(&world, warden, "oil_flask"), 3, "whole, at once");
    assert_eq!(picked(&world), 1, "`PickedUp` as today");
    step(&mut world, &mut schedule);
    assert!(!world.is_alive(flasks), "ended as a pickup is");
    let uid = inventory(&world, warden).unwrap().cell(0).unwrap().uid();
    assert_ne!(uid, 0, "the bag gave it a uid");
}

#[test]
fn a_full_bag_leaves_it_where_it_lies() {
    let (mut world, mut schedule) = rig();
    let warden = collector(&mut world, 0.0, 1);
    give(&mut world, warden, "stone", 1);
    let flasks = lying(&mut world, 2.0, Item::new("oil_flask").with_count(2));
    for _ in 0..5 {
        step(&mut world, &mut schedule);
    }
    assert!(world.is_alive(flasks) && !world.get::<Pickup>(flasks).unwrap().is_collected());
    assert_eq!((held(&world, warden, "oil_flask"), picked(&world)), (0, 0));
    // Room made, it goes in.
    crate::take(&mut world, warden, "stone", 1);
    step(&mut world, &mut schedule);
    assert_eq!(held(&world, warden, "oil_flask"), 2);
}

#[test]
fn two_items_in_one_step_never_claim_one_cell() {
    let (mut world, mut schedule) = rig();
    let warden = collector(&mut world, 0.0, 1);
    let first = lying(&mut world, 1.0, Item::new("wand"));
    let second = lying(&mut world, -1.0, Item::new("wand").with_rarity("magic"));
    step(&mut world, &mut schedule);
    assert_eq!(held(&world, warden, "wand"), 1, "one cell, one wand");
    let collected = [first, second]
        .iter()
        .filter(|&&entity| world.get::<Pickup>(entity).is_none_or(Pickup::is_collected))
        .count();
    assert_eq!(collected, 1, "the other still lies there");
    assert_eq!(picked(&world), 1);
}

#[test]
fn its_dropper_is_refused_for_a_second() {
    let (mut world, mut schedule) = rig();
    let warden = collector(&mut world, 0.0, 4);
    give(&mut world, warden, "oil_flask", 4);
    let dropped = drop_item(&mut world, warden, 0, 2).unwrap();
    assert_eq!(
        world.get::<GroundItem>(dropped).unwrap().refused(),
        Some(warden)
    );
    assert_eq!(held(&world, warden, "oil_flask"), 2);
    let mut frames = 0;
    while world
        .get::<Pickup>(dropped)
        .is_some_and(|pickup| !pickup.is_collected())
    {
        step(&mut world, &mut schedule);
        frames += 1;
        assert!(frames < 120, "never taken back");
    }
    assert!(
        (59..=61).contains(&frames),
        "taken back after a second: {frames} frames"
    );
    assert_eq!(held(&world, warden, "oil_flask"), 4);
    // Another collector takes it at once.
    let other = collector(&mut world, 40.0, 4);
    let near = world.get::<Position>(other).unwrap().0;
    let again = drop_item(&mut world, warden, 0, 1).unwrap();
    world.get_mut::<Position>(again).unwrap().0 = near;
    world.get_mut::<PrevPosition>(again).unwrap().0 = near;
    step(&mut world, &mut schedule);
    assert_eq!(held(&world, other, "oil_flask"), 1);
}

#[test]
fn the_label_takes_its_raritys_theme_style_or_falls_back() {
    let (mut world, mut schedule) = rig();
    let mut theme = Theme::default();
    theme.names.text_styles.insert(
        "rarity.rare".to_owned(),
        NamedStyle {
            text: TextOverrides {
                family: Some("serif".to_owned()),
                size: Some(18.0),
                color: Some([255, 210, 60, 255]),
                ..TextOverrides::default()
            },
            outline: Some(TextOutline::new([40, 20, 0, 255], 2.0)),
            shadow: Some(TextShadow::new([0, 0, 0, 160], Vec2::new(1.0, 1.0))),
            shrink: None,
        },
    );
    let mut tree = UiTree::new();
    tree.set_theme(theme.clone());
    world.insert_resource(tree);
    world
        .spawn_prefab("wand_drop", Vec2::new(100.0, 0.0))
        .unwrap();
    let plain = world
        .spawn_prefab("flask_drop", Vec2::new(200.0, 0.0))
        .unwrap();
    world
        .get_mut::<GroundItem>(plain)
        .unwrap()
        .item
        .set_count(3);
    let quiet = lying(&mut world, 300.0, Item::new("stone"));
    world.insert(quiet, GroundItem::new(Item::new("stone")).unlabelled());
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    let labels: Vec<WorldLabel> = world.resource_mut::<WorldLabels>().drain().collect();
    assert_eq!(labels.len(), 2, "the unlabelled stone shows none");
    let (wand, flask) = (&labels[0], &labels[1]);
    assert_eq!(wand.text, "Wand");
    assert_eq!(
        (wand.font_id.as_str(), wand.font_size, wand.color),
        ("serif", 18.0, [255, 210, 60, 255])
    );
    assert_eq!(wand.outline, Some(TextOutline::new([40, 20, 0, 255], 2.0)));
    assert!(wand.shadow.is_some());
    assert_eq!(flask.text, "Oil Flask \u{d7}3", "its count past one");
    let base = theme.base_text();
    assert_eq!(
        (flask.font_id.clone(), flask.font_size, flask.color),
        (base.family, base.size, base.color)
    );
    assert_eq!(
        flask.outline,
        Some(OUTLINE),
        "no `rarity.normal`: the base text, outlined"
    );
    assert_eq!(flask.offset, Vec2::new(0.0, -12.0));
    let centre = centre_of(&world, plain).unwrap();
    assert_eq!(flask.anchor, centre, "at its centre");
}

#[test]
fn dropping_spawns_the_bases_ground_prefab_holding_the_item() {
    let (mut world, _) = rig();
    let warden = collector(&mut world, 50.0, 4);
    give(&mut world, warden, "oil_flask", 3);
    give(&mut world, warden, "stone", 1);
    let dropped = drop_item(&mut world, warden, 0, 2).unwrap();
    let ground = world.get::<GroundItem>(dropped).unwrap();
    assert_eq!(
        (ground.item().base(), ground.item().count()),
        ("oil_flask", 2)
    );
    assert_eq!(
        ground.item().uid(),
        0,
        "a split stack's part has no uid until a bag takes it"
    );
    assert!(
        world.get::<Pickup>(dropped).is_some(),
        "the prefab's pickup"
    );
    assert_eq!(
        world.get::<Position>(dropped).unwrap().0,
        centre_of(&world, warden).unwrap()
    );
    assert_eq!(held(&world, warden, "oil_flask"), 1);
    assert!(
        drop_item(&mut world, warden, 1, 1).is_none(),
        "a stone has no `ground`"
    );
    assert_eq!(held(&world, warden, "stone"), 1, "and stays");
    assert!(
        drop_item(&mut world, warden, 3, 1).is_none(),
        "an empty cell"
    );
    assert!(drop_item(&mut world, warden, 0, 0).is_none(), "none of it");
    let whole = drop_item(&mut world, warden, 0, 9).unwrap();
    assert_eq!(
        world.get::<GroundItem>(whole).unwrap().item().count(),
        1,
        "at most what the cell holds"
    );
    assert!(inventory(&world, warden).unwrap().cell(0).is_none());
}

#[test]
fn a_ground_item_saves_its_item_whole() {
    let rig_with_source = || {
        let (mut world, schedule) = rig();
        let source = world.spawn_with((Transform::default(),));
        saves::name(&mut world, source, None, "source").unwrap();
        (world, schedule, source)
    };
    let (mut world, _, source) = rig_with_source();
    let prepared = world.prepare_prefab("wand_drop", [8.0, 8.0]).unwrap();
    world
        .resource_mut::<CommandBuffer>()
        .spawn_prefab_from(source, prepared);
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    let wand = world
        .query::<(Entity, &GroundItem)>()
        .map(|(entity, _)| entity)
        .next()
        .unwrap();
    let rolled = Item::new("wand")
        .with_rarity("rare")
        .with_level(12)
        .with_name(["Ember", "Coil"]);
    world.get_mut::<GroundItem>(wand).unwrap().item = rolled.clone();
    let form = serde_json::to_value(world.get::<GroundItem>(wand).unwrap()).unwrap();
    assert_eq!(form["item"], serde_json::to_value(&rolled).unwrap());
    assert_eq!(
        (form["label"].clone(), form["offset"].clone()),
        (json!(true), json!([0.0, 0.0]))
    );
    let saved = capture_world(&world).unwrap();
    let (mut fresh, ..) = rig_with_source();
    restore_world(&mut fresh, &saved, &(Restored,)).unwrap();
    let back: Vec<Item> = fresh
        .query::<(Entity, &GroundItem)>()
        .map(|(_, ground)| ground.item().clone())
        .collect();
    assert_eq!(back, [rolled], "the item whole");
    // The scene form takes a base's ID with a count, or an item alone.
    let components = world.resource::<tungsten_core::ComponentRegistry>();
    components
        .check_in(
            GROUND_ITEM,
            &json!({ "item": "oil_flask", "count": 4 }),
            &world,
        )
        .unwrap();
    let error = components
        .check_in(
            GROUND_ITEM,
            &json!({ "item": { "base": "oil_flask" }, "count": 4 }),
            &world,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("`count` goes beside a base's ID"), "{error}");
    let error = components
        .check_in(
            GROUND_ITEM,
            &json!({ "item": "oil_flask", "count": 9 }),
            &world,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("item.count"), "{error}");
}

#[test]
fn a_world_without_a_ground_item_queries_nothing() {
    let (mut world, mut schedule) = rig();
    let warden = collector(&mut world, 0.0, 4);
    let coin = world.spawn_with((
        Position(Vec2::new(2.0, 0.0)),
        PrevPosition(Vec2::new(2.0, 0.0)),
        Pickup::new(HitShape::circle(4.0)),
    ));
    for _ in 0..3 {
        step(&mut world, &mut schedule);
        schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    }
    assert!(!world.has_any::<GroundItem>());
    assert!(
        world.get::<Pickup>(coin).is_none_or(Pickup::is_collected),
        "a plain pickup as before"
    );
    assert_eq!(held(&world, warden, "oil_flask"), 0);
    assert!(
        world.resource::<WorldLabels>().is_empty(),
        "no label pushed"
    );
}
