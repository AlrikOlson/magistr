use super::*;

#[test]
fn a_mark_is_a_value_the_compositor_can_key_a_cache_on() {
    use std::collections::HashSet;
    let a = Mark::Icon {
        icon: Icon::Done,
        col: 0,
        cols: 1,
        ink: Color::Rgb(1, 2, 3),
    };
    let b = Mark::Icon {
        icon: Icon::Done,
        col: 1,
        cols: 1,
        ink: Color::Rgb(1, 2, 3),
    };
    let mut seen = HashSet::new();
    assert!(seen.insert(a.clone()));
    assert!(!seen.insert(a), "the same mark keys the same entry");
    assert!(seen.insert(b), "a mark one cell over is another entry");
}
