use super::*;

/// A model that was never told otherwise is a terminal. This is what
/// keeps every terminal golden, every cell harness and every `.snap`
/// unmoved by the existence of a second arrangement.
#[test]
fn the_default_surface_is_the_terminal() {
    assert_eq!(Surface::default(), Surface::Terminal);
    assert!(!Surface::default().is_native());
}

#[test]
fn only_the_window_is_native() {
    assert!(Surface::Native.is_native());
    assert!(!Surface::Terminal.is_native());
}
