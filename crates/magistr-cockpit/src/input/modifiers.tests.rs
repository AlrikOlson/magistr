use super::*;

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
