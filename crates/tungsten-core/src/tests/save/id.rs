use super::*;

#[test]
fn authored_ids_are_d222s_hash_of_the_name() {
    let player = SaveId::authored("player");
    assert_eq!(player.0, 2_895_758_080);
    assert!(player.is_authored());
    assert_eq!(
        SaveId::authored("player"),
        player,
        "the same name, the same ID"
    );
    assert_ne!(SaveId::authored("gem_1"), player);
    assert!(SaveId::authored("").is_authored());
}

#[test]
fn runtime_ids_count_below_bit_31_and_never_reach_it() {
    let mut state = SaveState::default();
    assert_eq!(state.take_id(), Some(SaveId(0)));
    assert_eq!(state.take_id(), Some(SaveId(1)));
    assert_eq!(state.next_id(), 2);
    assert!(!SaveId(1).is_authored());

    state.next_id = AUTHORED - 2;
    assert_eq!(state.take_id(), Some(SaveId(AUTHORED - 2)));
    let last = state.take_id().unwrap();
    assert_eq!(last, SaveId(AUTHORED - 1));
    assert!(!last.is_authored());
    assert_eq!(state.take_id(), None, "the count stops below bit 31");
    assert_eq!(state.take_id(), None);
    assert_eq!(state.next_id(), AUTHORED);
}

#[test]
fn a_name_a_live_entity_holds_is_refused_and_a_dead_ones_is_free() {
    let mut world = World::new();
    let first = world.spawn();
    let id = name(&mut world, first, Some("sample"), "bat_1").unwrap();
    assert_eq!(id, SaveId::authored("bat_1"));
    let saved = world.get::<Saved>(first).unwrap();
    assert_eq!(saved.id(), id);
    assert_eq!(saved.area(), Some("sample"));
    assert_eq!(saved.prefab(), None);

    let second = world.spawn();
    assert_eq!(
        name(&mut world, second, Some("sample"), "bat_1"),
        Err(NameError::Taken {
            name: "bat_1".into()
        })
    );
    assert!(
        !world.has::<Saved>(second),
        "a refused naming changes nothing"
    );
    assert_eq!(
        name(&mut world, first, Some("sample"), "bat_2"),
        Err(NameError::AlreadySaved {
            name: "bat_2".into(),
            id
        })
    );
    assert_eq!(name(&mut world, second, None, ""), Err(NameError::Empty));

    world.despawn(first);
    assert_eq!(
        name(&mut world, first, None, "bat_9"),
        Err(NameError::Dead {
            name: "bat_9".into()
        })
    );
    assert_eq!(
        name(&mut world, second, Some("sample"), "bat_1"),
        Ok(id),
        "the dead one's name is free"
    );
    let state = world.resource::<SaveState>();
    assert_eq!(state.named_entity(id), Some(second));
    assert_eq!(state.name_of(id), Some("bat_1"));
}

#[test]
fn a_world_where_nothing_was_named_has_no_save_state() {
    let mut world = World::new();
    world.register_saved::<u32>("count");
    let entity = world.spawn_with((7_u32,));
    world.despawn(entity);
    world.spawn_with((8_u32,));
    assert!(!saving(&world));
    assert!(!world.has_resource::<SaveState>());

    let named = world.spawn();
    name(&mut world, named, None, "player").unwrap();
    assert!(saving(&world));
}

#[test]
fn order_keys_put_saved_entities_first_by_id_then_the_rest_by_index() {
    let mut world = World::new();
    let plain_low = world.spawn();
    let late = world.spawn();
    let early = world.spawn();
    let plain_high = world.spawn();
    name(&mut world, late, None, "player").unwrap();
    name(&mut world, early, None, "gem_1").unwrap();
    let mut entities = vec![plain_high, late, plain_low, early];
    entities.sort_by_key(|&entity| order_key(&world, entity));

    let mut saved = [
        (SaveId::authored("player"), late),
        (SaveId::authored("gem_1"), early),
    ];
    saved.sort_by_key(|&(id, _)| id);
    assert_eq!(
        entities,
        [saved[0].1, saved[1].1, plain_low, plain_high],
        "saved ones by ID, then the rest by index"
    );
    assert_eq!(
        order_key(&world, plain_high),
        (true, u64::from(plain_high.id()))
    );
    assert_eq!(
        order_key(&world, late),
        (false, SaveId::authored("player").0)
    );
}

#[test]
fn naming_an_entity_picks_its_variant_again() {
    use glam::Vec2;

    use crate::assets::AssetRegistry;
    use crate::components::{Sprite, SpriteVariants};
    let mut assets = AssetRegistry::new();
    let pool: Vec<_> = ["crate", "crate_b", "crate_c"]
        .map(|name| assets.intern_sprite(name))
        .into();
    let pool = SpriteVariants::new(pool);
    let mut world = World::new();
    world.insert_resource(assets);
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..12 {
        let at = Vec2::new(i as f32 * 16.0, 0.0);
        let entity = world.spawn_with((Sprite::new(pool.by_position(at)), pool.clone()));
        let before = world.counters().archetype_moves;
        let id = name(&mut world, entity, Some("cave"), &format!("crate_{i}")).unwrap();
        assert_eq!(
            world.counters().archetype_moves - before,
            1,
            "the `Saved` insert alone: the pick moves nothing"
        );
        let shown = world.get::<Sprite>(entity).unwrap().asset_id;
        assert_eq!(shown, pool.by_save_id(id));
        seen.insert(shown.index());
    }
    assert_eq!(seen.len(), 3, "{seen:?}");
    // An entity with no pool keeps its sprite.
    let lamp = pool.pool()[1];
    let plain = world.spawn_with((Sprite::new(lamp),));
    name(&mut world, plain, Some("cave"), "lamp_1").unwrap();
    assert_eq!(world.get::<Sprite>(plain).unwrap().asset_id, lamp);
}
