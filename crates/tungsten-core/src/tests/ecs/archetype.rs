use super::*;

/// A type-key entry and its column, holding `values`.
fn column<T: 'static>(values: Vec<T>) -> (TypeId, Box<dyn AnyColumn>) {
    (TypeId::of::<T>(), Box::new(TypedVec(values)))
}

/// Archetype over `columns`, sorted into type-key order as storage creates them.
fn make_arch(mut columns: Vec<(TypeId, Box<dyn AnyColumn>)>) -> Archetype {
    columns.sort_by_key(|&(type_id, _)| type_id);
    let (types, columns): (Vec<_>, Vec<_>) = columns.into_iter().unzip();
    Archetype::new(types.into_boxed_slice(), columns)
}

fn push_row<A: 'static, B: 'static>(arch: &mut Archetype, entity: Entity, a: A, b: B) {
    arch.typed_column_mut::<A>().unwrap().0.push(a);
    arch.typed_column_mut::<B>().unwrap().0.push(b);
    arch.entities.push(entity);
}

fn values<T: 'static + Clone>(arch: &Archetype) -> Vec<T> {
    arch.typed_column::<T>().unwrap().0.clone()
}

fn make_entity(index: u32) -> Entity {
    Entity {
        index,
        generation: 0,
    }
}

#[test]
fn push_and_get() {
    let mut arch = make_arch(vec![column::<u32>(vec![]), column::<f32>(vec![])]);
    push_row::<u32, f32>(&mut arch, make_entity(0), 42u32, 1.5f32);

    assert_eq!(values::<u32>(&arch), vec![42u32]);
    assert_eq!(values::<f32>(&arch), vec![1.5f32]);
}

#[test]
fn columns_follow_the_sorted_type_key() {
    let arch = make_arch(vec![
        column::<u32>(vec![]),
        column::<f32>(vec![]),
        column::<bool>(vec![]),
    ]);

    assert!(arch.component_types.is_sorted());
    for (index, &type_id) in arch.component_types.iter().enumerate() {
        assert_eq!(arch.column_index(type_id), Some(index));
    }
    assert!(arch.typed_column::<u32>().is_some());
    assert!(arch.typed_column::<f32>().is_some());
    assert!(arch.typed_column::<bool>().is_some());
    assert!(arch.typed_column::<i64>().is_none());
    assert_eq!(arch.column_index(TypeId::of::<i64>()), None);
}

/// An archetype over the `[u8; K]` types whose bit is set in `mask`.
fn wide_arch(mask: u64) -> Archetype {
    let mut columns = Vec::new();
    macro_rules! widths {
        ($($k:literal)*) => {
            $(
                if mask & (1 << $k) != 0 {
                    columns.push(column::<[u8; $k]>(vec![[$k; $k]]));
                }
            )*
        };
    }
    widths!(1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32);
    make_arch(columns)
}

/// What `slot_column::<[u8; K]>` holds for every `K`, as a bit mask.
fn slot_mask(arch: &Archetype) -> u64 {
    let mut found = 0;
    macro_rules! widths {
        ($($k:literal)*) => {
            $(
                if let Some(column) = arch.slot_column::<[u8; $k]>() {
                    assert_eq!(column.0, vec![[$k; $k]]);
                    found |= 1 << $k;
                }
            )*
        };
    }
    widths!(1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32);
    found
}

#[test]
fn slot_column_finds_every_key_type_and_no_other() {
    // Keys of every size up to 32 types: a type in the key is found in its
    // slot, and a type outside it is rejected whether its slot is empty or
    // taken by another type.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for round in 0..200 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let mask = match round {
            0 => 0,
            1 => 0x1_FFFF_FFFE,
            _ => state & 0x1_FFFF_FFFE & (state >> 20 | state >> 37),
        };
        let mut arch = wide_arch(mask);
        assert_ne!(arch.column_slots.shift, ColumnSlots::NO_SHIFT);
        assert_eq!(slot_mask(&arch), mask, "mask {mask:#x}");
        // The mutable lookup takes the same path.
        assert_eq!(
            arch.slot_column_mut::<[u8; 5]>().is_some(),
            mask & (1 << 5) != 0
        );
        assert!(arch.slot_column::<u64>().is_none());
    }
}

#[test]
fn column_slots_give_every_hash_its_own_slot() {
    let hashes: Vec<u64> = (1..=40u64)
        .map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .collect();
    let slots = ColumnSlots::new(&hashes);

    assert_ne!(slots.shift, ColumnSlots::NO_SHIFT);
    for (index, &hash) in hashes.iter().enumerate() {
        assert_eq!(
            slots.slots[ColumnSlots::slot(hash, slots.shift)],
            index as u8
        );
    }
    let used = slots
        .slots
        .iter()
        .filter(|&&slot| slot != ColumnSlots::EMPTY)
        .count();
    assert_eq!(used, hashes.len());
}

#[test]
fn column_slots_fall_back_when_no_shift_separates_the_key() {
    // Two types with one hash collide under every shift, and a key longer
    // than the slot index can name has no table either.
    assert_eq!(ColumnSlots::new(&[7, 7]).shift, ColumnSlots::NO_SHIFT);
    let long: Vec<u64> = (0..300).collect();
    assert_eq!(ColumnSlots::new(&long).shift, ColumnSlots::NO_SHIFT);
    assert_ne!(ColumnSlots::new(&[]).shift, ColumnSlots::NO_SHIFT);
}

#[test]
fn slot_column_scans_the_key_without_a_slot_table() {
    let mask = 0b1011_0110;
    let mut arch = wide_arch(mask);
    arch.column_slots = ColumnSlots::new(&[7, 7]);

    assert_eq!(slot_mask(&arch), mask);
    assert!(arch.slot_column_mut::<[u8; 2]>().is_some());
    assert!(arch.slot_column_mut::<[u8; 3]>().is_none());
    assert!(
        arch.slot_column_erased_mut(TypeId::of::<[u8; 3]>())
            .is_none()
    );
}

#[test]
fn slot_column_erased_is_the_key_types_own_column() {
    let mut arch = wide_arch(0b1011_0110);

    let column = arch
        .slot_column_erased_mut(TypeId::of::<[u8; 4]>())
        .unwrap();
    assert_eq!(column.typed::<[u8; 4]>().unwrap().0, vec![[4; 4]]);
    assert!(column.typed::<[u8; 5]>().is_none());
}

#[test]
fn typed_rejects_another_component_type() {
    let (_, column) = column::<u32>(vec![7]);

    assert_eq!(column.typed::<u32>().unwrap().0, vec![7]);
    assert!(column.typed::<f32>().is_none());
}

#[test]
fn swap_remove_row_middle() {
    let mut arch = make_arch(vec![column::<u32>(vec![]), column::<f32>(vec![])]);

    let e0 = make_entity(0);
    let e1 = make_entity(1);
    let e2 = make_entity(2);

    push_row::<u32, f32>(&mut arch, e0, 0u32, 0.0f32);
    push_row::<u32, f32>(&mut arch, e1, 1u32, 1.0f32);
    push_row::<u32, f32>(&mut arch, e2, 2u32, 2.0f32);

    let displaced = arch.swap_remove_row(1);
    assert_eq!(displaced, Some(e2));
    assert_eq!(arch.entities.len(), 2);
    assert_eq!(arch.entities[1], e2);

    assert_eq!(values::<u32>(&arch), vec![0u32, 2u32]);
    assert_eq!(values::<f32>(&arch), vec![0.0f32, 2.0f32]);
}

#[test]
fn swap_remove_last_row_returns_none() {
    let mut arch = make_arch(vec![column::<u32>(vec![99u32])]);
    arch.entities = vec![make_entity(0)];

    let displaced = arch.swap_remove_row(0);
    assert_eq!(displaced, None);
    assert_eq!(arch.entities.len(), 0);
    assert!(values::<u32>(&arch).is_empty());
}

#[test]
fn move_components_to_transfers_matching_types() {
    let mut src = make_arch(vec![
        column::<u32>(vec![10u32, 20u32]),
        column::<f32>(vec![1.0f32, 2.0f32]),
        column::<i32>(vec![-1i32, -2i32]),
    ]);
    src.entities = vec![make_entity(0), make_entity(1)];

    let mut dst = make_arch(vec![column::<u32>(vec![]), column::<f32>(vec![])]);
    src.move_components_to(0, &mut dst);

    assert_eq!(values::<u32>(&dst), vec![10u32]);
    assert_eq!(values::<f32>(&dst), vec![1.0f32]);

    // Non-matching i32 column stays in source.
    assert_eq!(
        values::<i32>(&src).len(),
        2,
        "i32 column should be untouched"
    );
    assert_eq!(values::<u32>(&src), vec![20u32]);
    assert_eq!(values::<f32>(&src), vec![2.0f32]);
}

#[test]
fn move_components_to_superset_leaves_the_new_column_to_the_caller() {
    // The insert direction: every source column moves, and the type only the
    // destination has gets no row.
    let mut src = make_arch(vec![
        column::<u32>(vec![1u32, 2u32]),
        column::<i32>(vec![-1i32, -2i32]),
    ]);
    let mut dst = make_arch(vec![
        column::<u32>(vec![9u32]),
        column::<f32>(vec![9.0f32]),
        column::<i32>(vec![-9i32]),
    ]);

    src.move_components_to(1, &mut dst);

    assert_eq!(values::<u32>(&dst), vec![9u32, 2u32]);
    assert_eq!(values::<i32>(&dst), vec![-9i32, -2i32]);
    assert_eq!(values::<f32>(&dst), vec![9.0f32]);
    assert_eq!(values::<u32>(&src), vec![1u32]);
    assert_eq!(values::<i32>(&src), vec![-1i32]);
}

#[test]
fn move_components_to_pairs_columns_whatever_the_type_order() {
    // The merge walk depends on how the two keys interleave; `TypeId` order
    // is not ours to pick, so cover every subset of four types both ways.
    fn build(mask: u32, rows: u8) -> Archetype {
        let fill = |scale: u8| (0..rows).map(|row| row * 4 + scale).collect::<Vec<u8>>();
        let mut columns = Vec::new();
        if mask & 1 != 0 {
            columns.push(column::<u8>(fill(0)));
        }
        if mask & 2 != 0 {
            columns.push(column::<[u8; 1]>(
                fill(1).into_iter().map(|v| [v]).collect(),
            ));
        }
        if mask & 4 != 0 {
            columns.push(column::<[u8; 2]>(
                fill(2).into_iter().map(|v| [v; 2]).collect(),
            ));
        }
        if mask & 8 != 0 {
            columns.push(column::<[u8; 3]>(
                fill(3).into_iter().map(|v| [v; 3]).collect(),
            ));
        }
        make_arch(columns)
    }
    fn len_of(arch: &Archetype, bit: u32) -> Option<usize> {
        match bit {
            1 => arch.typed_column::<u8>().map(|col| col.0.len()),
            2 => arch.typed_column::<[u8; 1]>().map(|col| col.0.len()),
            4 => arch.typed_column::<[u8; 2]>().map(|col| col.0.len()),
            _ => arch.typed_column::<[u8; 3]>().map(|col| col.0.len()),
        }
    }

    for src_mask in 0..16u32 {
        for dst_mask in 0..16u32 {
            let mut src = build(src_mask, 2);
            let mut dst = build(dst_mask, 1);
            src.move_components_to(0, &mut dst);
            for bit in [1, 2, 4, 8] {
                let shared = src_mask & dst_mask & bit != 0;
                if src_mask & bit != 0 {
                    let expected = if shared { 1 } else { 2 };
                    assert_eq!(
                        len_of(&src, bit),
                        Some(expected),
                        "{src_mask} -> {dst_mask}"
                    );
                }
                if dst_mask & bit != 0 {
                    let expected = if shared { 2 } else { 1 };
                    assert_eq!(
                        len_of(&dst, bit),
                        Some(expected),
                        "{src_mask} -> {dst_mask}"
                    );
                }
            }
            if src_mask & dst_mask & 1 != 0 {
                // Row 0 moved; the swap-remove left row 1 behind.
                assert_eq!(values::<u8>(&dst), vec![0u8, 0u8]);
                assert_eq!(values::<u8>(&src), vec![4u8]);
            }
        }
    }
}

#[test]
fn moved_and_removed_rows_drop_exactly_once() {
    use std::rc::Rc;

    let tracker = Rc::new(());
    let mut src = make_arch(vec![column::<Rc<()>>(vec![
        tracker.clone(),
        tracker.clone(),
    ])]);
    src.entities = vec![make_entity(0), make_entity(1)];
    let mut dst = make_arch(vec![column::<Rc<()>>(vec![])]);
    assert_eq!(Rc::strong_count(&tracker), 3);

    // A move neither drops nor duplicates the value.
    src.move_components_to(0, &mut dst);
    assert_eq!(Rc::strong_count(&tracker), 3);
    assert_eq!(values::<Rc<()>>(&dst).len(), 1);

    // A despawn drops it in place.
    src.entities.swap_remove(0);
    src.swap_remove_row(0);
    assert_eq!(Rc::strong_count(&tracker), 2);

    drop(dst);
    assert_eq!(Rc::strong_count(&tracker), 1);
}

#[test]
#[should_panic(expected = "move_row_to: column type mismatch")]
fn move_row_to_rejects_another_column_type() {
    let (_, mut source) = column::<u32>(vec![1]);
    let (_, mut dest) = column::<f32>(vec![]);
    source.move_row_to(0, &mut *dest);
}

#[test]
fn columns_consistent_length_after_multiple_removals() {
    let mut arch = make_arch(vec![column::<u32>(vec![]), column::<bool>(vec![])]);

    for i in 0u32..5 {
        push_row::<u32, bool>(&mut arch, make_entity(i), i, i % 2 == 0);
    }

    arch.swap_remove_row(2);
    arch.swap_remove_row(0);

    let u32_len = arch.typed_column::<u32>().unwrap().0.len();
    let bool_len = arch.typed_column::<bool>().unwrap().0.len();
    assert_eq!(u32_len, arch.entities.len());
    assert_eq!(bool_len, arch.entities.len());
    assert_eq!(u32_len, 3);
}

#[test]
fn type_id_map_keys_distinct_types_distinctly() {
    use std::hash::BuildHasher;

    let build = std::hash::BuildHasherDefault::<TypeIdHasher>::default();
    let ids = [
        TypeId::of::<u8>(),
        TypeId::of::<u32>(),
        TypeId::of::<f32>(),
        TypeId::of::<String>(),
        TypeId::of::<Entity>(),
    ];
    let hashes: std::collections::HashSet<u64> = ids.iter().map(|id| build.hash_one(id)).collect();
    assert_eq!(hashes.len(), ids.len());

    let mut map: TypeIdMap<usize> = TypeIdMap::default();
    for (i, id) in ids.iter().enumerate() {
        map.insert(*id, i);
    }
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(map.get(id), Some(&i));
    }
}

#[test]
fn type_id_hashes_through_the_u64_fast_path() {
    // `TypeIdHasher::write` is only a fallback; if a toolchain starts hashing
    // `TypeId` through bytes, lookups stay correct but the pass-through is
    // lost, so this pins the assumption.
    use std::hash::Hash;

    #[derive(Default)]
    struct Probe {
        u64_writes: usize,
        byte_writes: usize,
    }
    impl Hasher for Probe {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, _bytes: &[u8]) {
            self.byte_writes += 1;
        }
        fn write_u64(&mut self, _n: u64) {
            self.u64_writes += 1;
        }
    }

    let mut probe = Probe::default();
    TypeId::of::<u32>().hash(&mut probe);
    assert_eq!((probe.u64_writes, probe.byte_writes), (1, 0));
}
