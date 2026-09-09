use super::*;

#[test]
fn new_and_as_str() {
    let id = SessionId::new("sess-123");
    assert_eq!(id.as_str(), "sess-123");
}

#[test]
fn display() {
    let id = SessionId::new("abc");
    assert_eq!(format!("{id}"), "abc");
}

#[test]
fn clone_is_equal() {
    let id = SessionId::new("x");
    let id2 = id.clone();
    assert_eq!(id, id2);
}

#[test]
fn hash_consistency() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SessionId::new("a"));
    set.insert(SessionId::new("a"));
    assert_eq!(set.len(), 1);
}
