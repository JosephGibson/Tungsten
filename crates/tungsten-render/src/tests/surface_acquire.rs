use super::{
    AcquireAction, AcquireStatus, MAX_ACQUIRE_ATTEMPTS, acquire_action, reconfigure_for_suboptimal,
};

const LAST: u32 = MAX_ACQUIRE_ATTEMPTS - 1;

#[test]
fn success_renders_without_reconfigure() {
    for attempt in 0..MAX_ACQUIRE_ATTEMPTS {
        assert_eq!(
            acquire_action(AcquireStatus::Success, attempt),
            AcquireAction::Render {
                reconfigure_after: false
            }
        );
    }
}

#[test]
fn suboptimal_renders_then_reconfigures_after_present() {
    assert_eq!(
        acquire_action(AcquireStatus::Suboptimal, 0),
        AcquireAction::Render {
            reconfigure_after: true
        }
    );
}

#[test]
fn timeout_and_occluded_skip_the_frame() {
    for status in [AcquireStatus::Timeout, AcquireStatus::Occluded] {
        assert_eq!(acquire_action(status, 0), AcquireAction::Skip);
        assert_eq!(acquire_action(status, LAST), AcquireAction::Skip);
    }
}

#[test]
fn outdated_reconfigures_once_then_skips() {
    assert_eq!(
        acquire_action(AcquireStatus::Outdated, 0),
        AcquireAction::ReconfigureAndRetry
    );
    assert_eq!(
        acquire_action(AcquireStatus::Outdated, LAST),
        AcquireAction::Skip
    );
}

#[test]
fn lost_recreates_once_then_fails_clearly() {
    assert_eq!(
        acquire_action(AcquireStatus::Lost, 0),
        AcquireAction::RecreateAndRetry
    );
    assert_eq!(
        acquire_action(AcquireStatus::Lost, LAST),
        AcquireAction::Fail
    );
}

#[test]
fn validation_fails() {
    assert_eq!(
        acquire_action(AcquireStatus::Validation, 0),
        AcquireAction::Fail
    );
}

#[test]
fn retries_are_bounded() {
    // Walking every status through every attempt never asks for a retry on
    // the last attempt, so a frame performs at most MAX_ACQUIRE_ATTEMPTS acquires.
    for status in [
        AcquireStatus::Success,
        AcquireStatus::Suboptimal,
        AcquireStatus::Timeout,
        AcquireStatus::Occluded,
        AcquireStatus::Outdated,
        AcquireStatus::Lost,
        AcquireStatus::Validation,
    ] {
        let action = acquire_action(status, LAST);
        assert!(
            !matches!(
                action,
                AcquireAction::ReconfigureAndRetry | AcquireAction::RecreateAndRetry
            ),
            "{status:?} retries on the last attempt"
        );
    }
}

#[test]
fn suboptimal_reconfigures_once_per_unchanged_size() {
    let size = (1280, 720);
    // First suboptimal frame at this size: reconfigure.
    assert!(reconfigure_for_suboptimal(size, size, None));
    // Still suboptimal at the same size: don't recreate the swapchain again.
    assert!(!reconfigure_for_suboptimal(size, size, Some(size)));
    // An earlier reconfigure at another size doesn't count.
    assert!(reconfigure_for_suboptimal(size, size, Some((800, 600))));
}

#[test]
fn suboptimal_follows_window_size_changes() {
    let configured = (1280, 720);
    assert!(reconfigure_for_suboptimal(
        (1600, 900),
        configured,
        Some(configured)
    ));
    // A zero-sized (minimized) window is not a usable new size.
    assert!(!reconfigure_for_suboptimal(
        (0, 0),
        configured,
        Some(configured)
    ));
}
