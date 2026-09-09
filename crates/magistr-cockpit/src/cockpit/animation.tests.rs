use super::*;

/// Every animation is in `ALL`, and `ALL` is in period order — finest
/// first, which is the order the set's reconciliation reads them in.
#[test]
fn all_holds_every_animation_finest_period_first() {
    assert_eq!(Animation::ALL.len(), 2);
    for pair in Animation::ALL.windows(2) {
        assert!(
            pair[0].period() <= pair[1].period(),
            "{:?} is coarser than {:?}",
            pair[0],
            pair[1]
        );
    }
}

/// The whole design rests on this: the tick count only means time
/// because the motion period is the smallest one declared, so a set
/// holding motion is reconciled to exactly it.
#[test]
fn motion_is_the_finest_period_so_ticks_keep_meaning_time() {
    for other in Animation::ALL {
        assert!(Animation::Motion.period() <= other.period());
    }
    assert_eq!(Animation::Clock.period().as_secs(), 1);
}

/// Each animation takes its own bit, so a set can hold any combination.
#[test]
fn every_animation_takes_its_own_bit() {
    let mut seen = 0_u8;
    for animation in Animation::ALL {
        assert_eq!(seen & animation.bit(), 0, "{animation:?} shares a bit");
        seen |= animation.bit();
    }
}
