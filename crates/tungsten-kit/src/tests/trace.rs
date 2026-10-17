use super::*;

fn event(name: &'static str, other: Option<Entity>, amount: Option<f32>) -> TraceKind {
    TraceKind::Event {
        name,
        other,
        amount,
    }
}

#[test]
fn the_ring_keeps_its_capacity_and_counts_what_it_dropped() {
    let mut world = World::new();
    let slime = world.spawn();
    world.insert_resource(KitTrace::new(2));
    for amount in [1.0, 2.0, 3.0] {
        trace(&mut world, slime, || event("damaged", None, Some(amount)));
    }
    let trace = world.resource::<KitTrace>();
    let amounts: Vec<Option<f32>> = trace
        .iter()
        .map(|entry| match entry.kind {
            TraceKind::Event { amount, .. } => amount,
            _ => None,
        })
        .collect();
    assert_eq!(
        amounts,
        [Some(2.0), Some(3.0)],
        "oldest first, the first dropped"
    );
    assert_eq!(trace.dropped(), 1);
}

#[test]
fn a_capacity_of_zero_records_nothing() {
    let mut world = World::new();
    let slime = world.spawn();
    world.insert_resource(KitTrace::new(0));
    trace(&mut world, slime, || event("killed", None, None));
    let trace = world.resource::<KitTrace>();
    assert_eq!((trace.iter().count(), trace.dropped()), (0, 1));
}

#[test]
fn without_a_trace_nothing_is_built() {
    let mut world = World::new();
    let slime = world.spawn();
    trace(&mut world, slime, || panic!("no trace, no entry"));
}

#[test]
fn entries_name_their_frame_and_the_steps_ended_before_them() {
    let mut world = World::new();
    let (zone, hero) = (world.spawn(), world.spawn());
    let mut time = Time::new();
    time.advance_frame(1.0 / 60.0);
    time.advance_frame(1.0 / 60.0);
    world.insert_resource(time);
    world.insert_resource(KitTrace::default());
    world.resource_mut::<KitTrace>().end_step();
    trace(&mut world, zone, || event("zone_entered", Some(hero), None));
    let trace = world.resource::<KitTrace>();
    let entry = trace.iter().next().unwrap();
    assert_eq!((entry.frame, entry.step, entry.entity), (2, 1, zone));
    assert_eq!(trace.steps(), 1);
    assert_eq!(trace.about(hero).count(), 1, "the other entity's too");
    assert_eq!(
        entry.to_string(),
        format!("frame 2 step 1: {zone} zone_entered with {hero}")
    );
}

#[test]
fn clear_drops_the_entries_and_the_count_and_keeps_the_steps() {
    let mut world = World::new();
    let slime = world.spawn();
    world.insert_resource(KitTrace::new(1));
    trace(&mut world, slime, || TraceKind::Despawn { by: "lifetime" });
    trace(&mut world, slime, || TraceKind::Despawn { by: "cues" });
    let trace = world.resource_mut::<KitTrace>();
    trace.end_step();
    trace.clear();
    assert_eq!(
        (trace.iter().count(), trace.dropped(), trace.steps()),
        (0, 0, 1)
    );
}

#[test]
fn each_kind_writes_one_line() {
    let mut world = World::new();
    let slime = world.spawn();
    let at = |kind: TraceKind| {
        TraceEntry {
            frame: 12,
            step: 40,
            entity: slime,
            kind,
        }
        .to_string()
    };
    let head = format!("frame 12 step 40: {slime}");
    assert_eq!(
        at(event("damaged", None, Some(3.0))),
        format!("{head} damaged 3")
    );
    assert_eq!(
        at(TraceKind::Cue {
            event: "killed",
            action: "counter",
            rule: Some("combat.0".to_string()),
        }),
        format!("{head} killed: counter (rule combat.0)")
    );
    assert_eq!(
        at(TraceKind::Cue {
            event: "damaged",
            action: "flash",
            rule: None,
        }),
        format!("{head} damaged: flash (own cue)")
    );
    assert_eq!(
        at(TraceKind::Skipped {
            event: "damaged",
            action: "all",
            why: "the subject is despawning",
        }),
        format!("{head} damaged: all skipped, the subject is despawning")
    );
    assert_eq!(
        at(TraceKind::Spawn {
            prefab: "puff".to_string(),
            by: "cues",
        }),
        format!("{head} spawned `puff` by cues")
    );
    assert_eq!(
        at(TraceKind::SpawnFailed {
            prefab: "ruby".to_string(),
            by: "spawner",
            error: "unknown prefab `ruby`".to_string(),
        }),
        format!("{head} `ruby` by spawner spawned nothing: unknown prefab `ruby`")
    );
    assert_eq!(
        at(TraceKind::Despawn { by: "lifetime" }),
        format!("{head} despawned by lifetime")
    );
}
