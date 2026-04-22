use super::*;

fn make_arch(id: ArchetypeId, types: &[TypeId]) -> Archetype {
    let mut sorted = types.to_vec();
    sorted.sort();
    Archetype::new(id, sorted.into_boxed_slice())
}

fn seed_columns<A: 'static, B: 'static>(arch: &mut Archetype) {
    arch.columns
        .insert(TypeId::of::<A>(), Box::new(TypedVec::<A>(Vec::new())));
    arch.columns
        .insert(TypeId::of::<B>(), Box::new(TypedVec::<B>(Vec::new())));
}

fn push_row<A: 'static, B: 'static>(arch: &mut Archetype, entity: Entity, a: A, b: B) {
    arch.columns
        .get_mut(&TypeId::of::<A>())
        .unwrap()
        .push_erased(Box::new(a));
    arch.columns
        .get_mut(&TypeId::of::<B>())
        .unwrap()
        .push_erased(Box::new(b));
    arch.entities.push(entity);
}

fn make_entity(index: u32) -> Entity {
    Entity {
        index,
        generation: 0,
    }
}

// ------------------------------------------------------------------

#[test]
fn push_and_get() {
    let mut arch = make_arch(1, &[TypeId::of::<u32>(), TypeId::of::<f32>()]);
    seed_columns::<u32, f32>(&mut arch);
    push_row::<u32, f32>(&mut arch, make_entity(0), 42u32, 1.5f32);

    let col = arch.columns[&TypeId::of::<u32>()]
        .as_any()
        .downcast_ref::<TypedVec<u32>>()
        .unwrap();
    assert_eq!(col.0[0], 42u32);
}

#[test]
fn swap_remove_row_middle() {
    let mut arch = make_arch(1, &[TypeId::of::<u32>(), TypeId::of::<f32>()]);
    seed_columns::<u32, f32>(&mut arch);

    let e0 = make_entity(0);
    let e1 = make_entity(1);
    let e2 = make_entity(2);

    push_row::<u32, f32>(&mut arch, e0, 0u32, 0.0f32);
    push_row::<u32, f32>(&mut arch, e1, 1u32, 1.0f32);
    push_row::<u32, f32>(&mut arch, e2, 2u32, 2.0f32);

    // Remove row 1 (e1). e2 should displace into row 1.
    let displaced = arch.swap_remove_row(1);
    assert_eq!(displaced, Some(e2));
    assert_eq!(arch.row_count(), 2);
    assert_eq!(arch.entities[1], e2);

    // Check column consistency.
    let u32_col = arch.columns[&TypeId::of::<u32>()]
        .as_any()
        .downcast_ref::<TypedVec<u32>>()
        .unwrap();
    assert_eq!(u32_col.0, vec![0u32, 2u32]);

    let f32_col = arch.columns[&TypeId::of::<f32>()]
        .as_any()
        .downcast_ref::<TypedVec<f32>>()
        .unwrap();
    assert_eq!(f32_col.0, vec![0.0f32, 2.0f32]);
}

#[test]
fn swap_remove_last_row_returns_none() {
    let mut arch = make_arch(1, &[TypeId::of::<u32>()]);
    arch.columns
        .insert(TypeId::of::<u32>(), Box::new(TypedVec::<u32>(Vec::new())));
    push_row::<u32, u32>(&mut arch, make_entity(0), 99u32, 99u32); // only one col
                                                                   // Manually fix: just one column
    arch.columns.clear();
    arch.columns
        .insert(TypeId::of::<u32>(), Box::new(TypedVec::<u32>(vec![99u32])));
    arch.entities = vec![make_entity(0)];

    let displaced = arch.swap_remove_row(0);
    assert_eq!(displaced, None);
    assert_eq!(arch.row_count(), 0);
}

#[test]
fn move_components_to_transfers_matching_types() {
    // Source: {u32, f32, i32}. Dest: {u32, f32} (removing i32).
    let mut src = make_arch(
        1,
        &[
            TypeId::of::<u32>(),
            TypeId::of::<f32>(),
            TypeId::of::<i32>(),
        ],
    );
    src.columns.insert(
        TypeId::of::<u32>(),
        Box::new(TypedVec::<u32>(vec![10u32, 20u32])),
    );
    src.columns.insert(
        TypeId::of::<f32>(),
        Box::new(TypedVec::<f32>(vec![1.0f32, 2.0f32])),
    );
    src.columns.insert(
        TypeId::of::<i32>(),
        Box::new(TypedVec::<i32>(vec![-1i32, -2i32])),
    );
    src.entities = vec![make_entity(0), make_entity(1)];

    let mut dst = make_arch(2, &[TypeId::of::<u32>(), TypeId::of::<f32>()]);
    // dst columns are initially empty (lazy creation).

    // Move row 0 (value 10 / 1.0 / -1).
    src.move_components_to(0, &mut dst);

    // dst should now have row 0.
    let dst_u32 = dst.columns[&TypeId::of::<u32>()]
        .as_any()
        .downcast_ref::<TypedVec<u32>>()
        .unwrap();
    assert_eq!(dst_u32.0, vec![10u32]);

    let dst_f32 = dst.columns[&TypeId::of::<f32>()]
        .as_any()
        .downcast_ref::<TypedVec<f32>>()
        .unwrap();
    assert_eq!(dst_f32.0, vec![1.0f32]);

    // i32 was NOT in dst.component_types, so it was left in src.
    let src_i32 = src.columns[&TypeId::of::<i32>()]
        .as_any()
        .downcast_ref::<TypedVec<i32>>()
        .unwrap();
    // src had 2 rows; after move_components_to(0, …) the moved columns are
    // now 1 row with the last element at index 0.
    assert_eq!(src_i32.0.len(), 2, "i32 column should be untouched");

    // src u32/f32 columns should each have 1 row remaining.
    let src_u32 = src.columns[&TypeId::of::<u32>()]
        .as_any()
        .downcast_ref::<TypedVec<u32>>()
        .unwrap();
    assert_eq!(src_u32.0, vec![20u32]);
}

#[test]
fn columns_consistent_length_after_multiple_removals() {
    let mut arch = make_arch(1, &[TypeId::of::<u32>(), TypeId::of::<bool>()]);
    seed_columns::<u32, bool>(&mut arch);

    for i in 0u32..5 {
        push_row::<u32, bool>(&mut arch, make_entity(i), i, i % 2 == 0);
    }

    // Remove rows 2, then 0.
    arch.swap_remove_row(2);
    arch.swap_remove_row(0);

    let u32_len = arch.columns[&TypeId::of::<u32>()].len();
    let bool_len = arch.columns[&TypeId::of::<bool>()].len();
    assert_eq!(u32_len, arch.entities.len());
    assert_eq!(bool_len, arch.entities.len());
    assert_eq!(u32_len, 3);
}
