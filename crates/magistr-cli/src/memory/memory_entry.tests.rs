use super::*;
use crate::memory::MemoryCategory;

#[test]
fn entry_new_no_timestamp() {
    let entry = MemoryEntry::new(MemoryCategory::Patterns, "uses builder pattern");
    assert_eq!(entry.category, MemoryCategory::Patterns);
    assert_eq!(entry.text, "uses builder pattern");
    assert!(entry.added_at.is_none());
}

#[test]
fn entry_now_has_timestamp() {
    let entry = MemoryEntry::now(MemoryCategory::Observations, "saw a bug");
    assert!(entry.added_at.is_some());
}
