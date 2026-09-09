use std::path::PathBuf;

use super::*;

#[test]
fn render_empty_memory() {
    let mem = ProjectMemory::new("test");
    let md = render(&mem);
    assert_eq!(md, "# Project Memory — test\n");
}

#[test]
fn render_with_entries() {
    let mut mem = ProjectMemory::new("my-project");
    mem.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "builder pattern",
    ));
    mem.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "newtype wrappers",
    ));
    mem.add(MemoryEntry::new(
        MemoryCategory::Mistakes,
        "no unwrap in lib",
    ));

    let md = render(&mem);
    assert!(md.contains("## Patterns"));
    assert!(md.contains("- builder pattern"));
    assert!(md.contains("- newtype wrappers"));
    assert!(md.contains("## Common Mistakes"));
    assert!(md.contains("- no unwrap in lib"));
}

#[test]
fn render_with_timestamped_entry() {
    let mut mem = ProjectMemory::new("proj");
    let entry = MemoryEntry {
        category: MemoryCategory::Observations,
        text: "found a bug".to_string(),
        added_at: Some(
            chrono::NaiveDate::from_ymd_opt(2026, 3, 11)
                .expect("valid date")
                .and_hms_opt(0, 0, 0)
                .expect("valid time")
                .and_utc(),
        ),
    };
    mem.add(entry);

    let md = render(&mem);
    assert!(md.contains("- [2026-03-11] found a bug"));
}

#[test]
fn parse_empty_string() {
    let mem = parse("");
    assert!(mem.is_empty());
    assert!(mem.project_name.is_empty());
}

#[test]
fn parse_title_only() {
    let mem = parse("# Project Memory — my-project\n");
    assert_eq!(mem.project_name, "my-project");
    assert!(mem.is_empty());
}

#[test]
fn parse_round_trip() {
    let mut mem = ProjectMemory::new("round-trip");
    mem.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "builder pattern",
    ));
    mem.add(MemoryEntry::new(
        MemoryCategory::Architecture,
        "monorepo with workspaces",
    ));
    mem.add(MemoryEntry::new(
        MemoryCategory::Conventions,
        "conventional commits",
    ));
    mem.add(MemoryEntry::new(
        MemoryCategory::Mistakes,
        "no clone to silence borrow checker",
    ));

    let md = render(&mem);
    let parsed = parse(&md);

    assert_eq!(parsed.project_name, "round-trip");
    assert_eq!(parsed.len(), 4);
    assert_eq!(parsed.by_category(MemoryCategory::Patterns).len(), 1);
    assert_eq!(parsed.by_category(MemoryCategory::Architecture).len(), 1);
    assert_eq!(parsed.by_category(MemoryCategory::Conventions).len(), 1);
    assert_eq!(parsed.by_category(MemoryCategory::Mistakes).len(), 1);
}

#[test]
fn parse_timestamped_entry() {
    let md = "# Project Memory — proj\n\n## Observations\n\n- [2026-03-11] found bug\n";
    let mem = parse(md);

    assert_eq!(mem.len(), 1);
    let entry = &mem.entries[0];
    assert_eq!(entry.category, MemoryCategory::Observations);
    assert_eq!(entry.text, "found bug");
    assert!(entry.added_at.is_some());
}

#[test]
fn parse_ignores_unknown_sections() {
    let md = "# Project Memory — proj\n\n## Unknown Section\n\n- some entry\n\n## Patterns\n\n- real entry\n";
    let mem = parse(md);

    assert_eq!(mem.len(), 1);
    assert_eq!(mem.entries[0].text, "real entry");
}

#[test]
fn parse_tolerates_extra_whitespace() {
    let md = "# Project Memory — proj\n\n## Patterns\n\n- entry one  \n  \n- entry two\n";
    let mem = parse(md);
    assert_eq!(mem.len(), 2);
}

#[test]
fn save_and_load_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join(".magistr").join("memory.md");

    let mut mem = ProjectMemory::new("test-project");
    mem.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "uses Result everywhere",
    ));
    mem.add_observation("learned something");

    save(&mem, &path).unwrap();
    let loaded = load(&path).unwrap();

    assert_eq!(loaded.project_name, "test-project");
    assert_eq!(loaded.by_category(MemoryCategory::Patterns).len(), 1);
    assert_eq!(loaded.by_category(MemoryCategory::Observations).len(), 1);
}

#[test]
fn load_missing_file_returns_empty() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.md");
    let mem = load(&path).unwrap();
    assert!(mem.is_empty());
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nested").join("deep").join("memory.md");

    let mem = ProjectMemory::new("proj");
    save(&mem, &path).unwrap();
    assert!(path.is_file());
}

#[test]
fn memory_path_resolves_correctly() {
    let path = memory_path(Path::new("/home/user/project"));
    assert_eq!(path, PathBuf::from("/home/user/project/.magistr/memory.md"));
}

#[test]
fn error_display() {
    let err = MemoryStorageError::Io(
        PathBuf::from("/tmp/test"),
        std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    );
    assert!(err.to_string().contains("memory I/O error"));
}

#[test]
fn render_categories_in_order() {
    let mut mem = ProjectMemory::new("proj");
    // Add in reverse order
    mem.add(MemoryEntry::new(MemoryCategory::Observations, "obs"));
    mem.add(MemoryEntry::new(MemoryCategory::Patterns, "pat"));

    let md = render(&mem);
    let pat_pos = md.find("## Patterns").unwrap();
    let obs_pos = md.find("## Observations").unwrap();
    assert!(
        pat_pos < obs_pos,
        "Patterns should come before Observations"
    );
}

#[test]
fn parse_title_with_dash_separator() {
    let mem = parse("# Project Memory - my-project\n");
    assert_eq!(mem.project_name, "my-project");
}
