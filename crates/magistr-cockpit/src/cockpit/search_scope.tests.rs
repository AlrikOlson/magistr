use super::*;

#[test]
fn the_chips_walk_in_a_ring_both_ways() {
    let mut scope = SearchScope::default();
    assert_eq!(scope, SearchScope::Everything);
    for expected in SearchScope::ALL.into_iter().skip(1) {
        scope = scope.next();
        assert_eq!(scope, expected);
    }
    assert_eq!(scope.next(), SearchScope::Everything, "wraps");
    for expected in SearchScope::ALL.into_iter().rev().skip(1) {
        scope = scope.prev();
        assert_eq!(scope, expected);
    }
}

#[test]
fn every_scope_but_everything_names_one_kind_and_its_label_is_that_kinds_word() {
    assert_eq!(SearchScope::Everything.kind(), None);
    for scope in SearchScope::ALL.into_iter().skip(1) {
        let kind = scope.kind().expect("a kind");
        assert_eq!(scope.label(), kind.word());
    }
    assert_eq!(SearchScope::at(0), Some(SearchScope::Everything));
    assert_eq!(SearchScope::at(4), Some(SearchScope::Sessions));
    assert_eq!(SearchScope::at(5), None);
}
