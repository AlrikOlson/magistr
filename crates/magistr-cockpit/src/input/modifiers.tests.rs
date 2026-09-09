use super::*;

#[test]
fn kitty_field_is_one_plus_bits() {
    assert_eq!(Modifiers::from_kitty(1), Modifiers::NONE);
    assert_eq!(Modifiers::from_kitty(0), Modifiers::NONE);
    assert_eq!(Modifiers::from_kitty(2), Modifiers::SHIFT);
    assert_eq!(Modifiers::from_kitty(5), Modifiers::CTRL);
    assert_eq!(Modifiers::from_kitty(6), Modifiers::SHIFT | Modifiers::CTRL);
    assert_eq!(Modifiers::from_kitty(9), Modifiers::SUPER);
}

#[test]
fn kitty_lock_and_hyper_bits_are_dropped() {
    assert_eq!(Modifiers::from_kitty(1 + 64 + 128), Modifiers::NONE);
    assert_eq!(Modifiers::from_kitty(1 + 16 + 32 + 4), Modifiers::CTRL);
}

#[test]
fn sgr_bits_map_shift_alt_ctrl() {
    assert_eq!(Modifiers::from_sgr(0), Modifiers::NONE);
    assert_eq!(Modifiers::from_sgr(4), Modifiers::SHIFT);
    assert_eq!(Modifiers::from_sgr(8), Modifiers::ALT);
    assert_eq!(Modifiers::from_sgr(16 + 32), Modifiers::CTRL);
    assert_eq!(
        Modifiers::from_sgr(28),
        Modifiers::SHIFT | Modifiers::ALT | Modifiers::CTRL
    );
}

#[test]
fn contains_and_is_empty() {
    let m = Modifiers::SHIFT | Modifiers::CTRL;
    assert!(m.contains(Modifiers::SHIFT));
    assert!(m.contains(Modifiers::SHIFT | Modifiers::CTRL));
    assert!(!m.contains(Modifiers::ALT));
    assert!(m.contains(Modifiers::NONE));
    assert!(!m.is_empty());
    assert!(Modifiers::NONE.is_empty());
    assert_eq!(m.bits(), 5);
}
