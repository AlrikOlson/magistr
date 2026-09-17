use super::*;

/// The same name is the same pointer — a second ask allocates nothing,
/// which is what bounds the interner by the number of distinct names.
#[test]
fn the_same_name_is_one_allocation() {
    let first = interned("a project the galaxy learned");
    let second = interned("a project the galaxy learned");
    assert_eq!(first, second);
    assert!(std::ptr::eq(first, second), "one string, not two copies");
    assert_ne!(interned("another"), first);
}
