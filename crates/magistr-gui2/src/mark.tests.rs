use super::*;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hashed(mark: Mark<'_>) -> u64 {
    let mut hasher = DefaultHasher::new();
    mark.hash(&mut hasher);
    hasher.finish()
}

/// A mark is its own atlas key, which is only possible because a sigil
/// is carried as the name it derives from.
#[test]
fn a_mark_is_hashable_and_two_marks_differ() {
    assert_eq!(hashed(Mark::Icon(Icon::Run)), hashed(Mark::Icon(Icon::Run)));
    assert_ne!(
        hashed(Mark::Icon(Icon::Run)),
        hashed(Mark::Icon(Icon::Edit))
    );
    assert_eq!(
        hashed(Mark::Sigil("magistr")),
        hashed(Mark::Sigil("magistr"))
    );
    assert_ne!(
        hashed(Mark::Sigil("magistr")),
        hashed(Mark::Sigil("tidewatch"))
    );
}

/// Both kinds of mark answer the same question the same way, so the
/// producer has one path and not two.
#[test]
fn both_kinds_of_mark_answer_with_shapes() {
    assert_eq!(
        Mark::Icon(Icon::Done).shapes(),
        Icon::Done.shapes().to_vec()
    );
    assert_eq!(
        Mark::Sigil("magistr").shapes(),
        Sigil::of("magistr").shapes()
    );
    assert!(
        !Mark::Sigil("").shapes().is_empty(),
        "even an unnamed project gets a mark"
    );
}
