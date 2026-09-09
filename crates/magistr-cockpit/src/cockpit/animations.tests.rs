use super::*;

/// The empty set is the idle guarantee: nothing is in it and it asks the
/// runtime for no interval at all.
#[test]
fn the_empty_set_asks_for_no_interval() {
    let none = Animations::NONE;
    assert!(none.is_empty());
    assert_eq!(none.period(), None);
    for animation in Animation::ALL {
        assert!(!none.contains(animation));
    }
}

/// A set holding one animation is reconciled to that animation's period.
#[test]
fn one_animation_reconciles_to_its_own_period() {
    for animation in Animation::ALL {
        let set = Animations::NONE.with(animation);
        assert!(!set.is_empty());
        assert!(set.contains(animation));
        assert_eq!(set.period(), Some(animation.period()));
    }
}

/// The whole point of the set: independent declarations become one
/// interval, the finest anyone asked for — so the clock at 1 Hz never
/// slows the motion layer's 60 ms.
#[test]
fn a_mixed_set_reconciles_to_the_finest_period() {
    let both = Animations::NONE
        .with(Animation::Clock)
        .with(Animation::Motion);
    assert_eq!(both.period(), Some(Animation::Motion.period()));
    assert!(both.contains(Animation::Clock));
    assert!(both.contains(Animation::Motion));
}

/// Declaring the same animation twice is the same set — the model
/// rebuilds it from scratch on every fold and must not accumulate.
#[test]
fn declaring_an_animation_twice_is_the_same_set() {
    let once = Animations::NONE.with(Animation::Clock);
    assert_eq!(once.with(Animation::Clock), once);
}

/// `with_if` is `with` under a condition and nothing under its negation.
#[test]
fn with_if_adds_only_when_live() {
    assert_eq!(
        Animations::NONE.with_if(Animation::Motion, true),
        Animations::NONE.with(Animation::Motion)
    );
    assert_eq!(
        Animations::NONE.with_if(Animation::Motion, false),
        Animations::NONE
    );
}
