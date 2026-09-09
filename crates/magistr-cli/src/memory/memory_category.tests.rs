use super::*;

#[test]
fn category_heading_round_trip() {
    for cat in MemoryCategory::ALL {
        let heading = cat.heading();
        let parsed = MemoryCategory::from_heading(heading);
        assert_eq!(parsed, Some(*cat), "failed for {cat}");
    }
}

#[test]
fn category_from_heading_unknown_returns_none() {
    assert_eq!(MemoryCategory::from_heading("Unknown"), None);
    assert_eq!(MemoryCategory::from_heading(""), None);
}

#[test]
fn category_display() {
    assert_eq!(MemoryCategory::Patterns.to_string(), "Patterns");
    assert_eq!(MemoryCategory::Mistakes.to_string(), "Common Mistakes");
}

#[test]
fn all_categories_has_five() {
    assert_eq!(MemoryCategory::ALL.len(), 5);
}
