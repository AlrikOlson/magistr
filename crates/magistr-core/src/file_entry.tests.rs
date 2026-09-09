use super::*;

#[test]
fn file_entry_defaults() {
    let entry = FileEntry {
        path: "src/main.rs".into(),
        depth: 1,
        is_dir: false,
        status: FileStatus::Modified,
        additions: Some(10),
        deletions: Some(3),
        is_agent_modified: true,
    };
    assert_eq!(entry.path, "src/main.rs");
    assert_eq!(entry.depth, 1);
    assert!(!entry.is_dir);
    assert!(entry.is_agent_modified);
    assert_eq!(entry.additions, Some(10));
    assert_eq!(entry.deletions, Some(3));
}

#[test]
fn directory_entry() {
    let dir = FileEntry {
        path: "src".into(),
        depth: 0,
        is_dir: true,
        status: FileStatus::Modified,
        additions: None,
        deletions: None,
        is_agent_modified: false,
    };
    assert!(dir.is_dir);
    assert!(!dir.is_agent_modified);
    assert!(dir.additions.is_none());
}
