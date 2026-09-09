use super::*;
use crate::memory::{MemoryCategory, MemoryEntry};

#[test]
fn memory_new_is_empty() {
    let mem = ProjectMemory::new("test-project");
    assert!(mem.is_empty());
    assert_eq!(mem.len(), 0);
    assert_eq!(mem.project_name, "test-project");
}

#[test]
fn memory_add_and_query() {
    let mut mem = ProjectMemory::new("proj");
    mem.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "builder pattern",
    ));
    mem.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "newtype wrappers",
    ));
    mem.add(MemoryEntry::new(MemoryCategory::Mistakes, "no unwrap"));

    assert_eq!(mem.len(), 3);
    assert_eq!(mem.by_category(MemoryCategory::Patterns).len(), 2);
    assert_eq!(mem.by_category(MemoryCategory::Mistakes).len(), 1);
    assert_eq!(mem.by_category(MemoryCategory::Architecture).len(), 0);
}

#[test]
fn memory_add_observation() {
    let mut mem = ProjectMemory::new("proj");
    mem.add_observation("discovered TDD workflow");

    assert_eq!(mem.len(), 1);
    let entries = mem.by_category(MemoryCategory::Observations);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].text, "discovered TDD workflow");
    assert!(entries[0].added_at.is_some());
}

#[test]
fn memory_retain() {
    let mut mem = ProjectMemory::new("proj");
    mem.add(MemoryEntry::new(MemoryCategory::Patterns, "keep"));
    mem.add(MemoryEntry::new(MemoryCategory::Mistakes, "remove"));
    mem.add(MemoryEntry::new(MemoryCategory::Patterns, "also keep"));

    mem.retain(|e| e.category == MemoryCategory::Patterns);
    assert_eq!(mem.len(), 2);
}

#[test]
fn category_counts() {
    let mut mem = ProjectMemory::new("proj");
    mem.add(MemoryEntry::new(MemoryCategory::Patterns, "a"));
    mem.add(MemoryEntry::new(MemoryCategory::Patterns, "b"));
    mem.add(MemoryEntry::new(MemoryCategory::Mistakes, "c"));

    let counts = mem.category_counts();
    assert_eq!(counts.len(), 2);
    assert!(counts.contains(&(MemoryCategory::Patterns, 2)));
    assert!(counts.contains(&(MemoryCategory::Mistakes, 1)));
}
