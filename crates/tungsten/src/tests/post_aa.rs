use super::*;
use tungsten_core::ecs::World;

#[test]
fn request_post_aa_writes_pending_when_absent() {
    let mut world = World::new();
    request_post_aa(&mut world, PostAaMode::SmaaHigh);
    let pending = world.get_resource::<PendingPostAa>().copied().unwrap();
    assert_eq!(pending.0, Some(PostAaMode::SmaaHigh));
}

#[test]
fn request_post_aa_replaces_pending() {
    let mut world = World::new();
    request_post_aa(&mut world, PostAaMode::SmaaLow);
    request_post_aa(&mut world, PostAaMode::SmaaUltra);
    let pending = world.get_resource::<PendingPostAa>().copied().unwrap();
    assert_eq!(pending.0, Some(PostAaMode::SmaaUltra));
}

#[test]
fn take_pending_post_aa_clears() {
    let mut world = World::new();
    request_post_aa(&mut world, PostAaMode::SmaaMedium);
    let taken = take_pending_post_aa(&mut world);
    assert_eq!(taken, Some(PostAaMode::SmaaMedium));
    let again = take_pending_post_aa(&mut world);
    assert!(again.is_none());
}

#[test]
fn sync_post_aa_state_writes_then_updates() {
    let mut world = World::new();
    sync_post_aa_state(&mut world, PostAaMode::SmaaHigh);
    let state = world.get_resource::<PostAaState>().copied().unwrap();
    assert_eq!(state.mode, PostAaMode::SmaaHigh);
    sync_post_aa_state(&mut world, PostAaMode::Off);
    let state = world.get_resource::<PostAaState>().copied().unwrap();
    assert_eq!(state.mode, PostAaMode::Off);
}
