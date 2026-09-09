use std::time::Duration;

use super::*;

/// A clock built the old way carries no wall-clock reading; the builder
/// adds one without touching the frame or the elapsed time.
#[test]
fn the_epoch_is_absent_unless_the_runtime_sets_it() {
    let clock = FrameClock::new(17, Duration::from_millis(3_200));
    assert_eq!(clock.epoch_secs(), None);
    let stamped = clock.with_epoch_secs(1_788_264_000);
    assert_eq!(stamped.epoch_secs(), Some(1_788_264_000));
    assert_eq!(stamped.frame(), 17);
    assert_eq!(stamped.elapsed(), Duration::from_millis(3_200));
    assert_ne!(
        clock, stamped,
        "the reading is part of the clock's identity"
    );
}
