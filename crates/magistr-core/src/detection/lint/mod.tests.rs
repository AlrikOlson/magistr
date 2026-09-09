use std::path::Path;

use super::*;

#[test]
fn lint_file_nonexistent() {
    let result = lint_file(Path::new("/nonexistent/CLAUDE.md"));
    assert!(result.is_err());
}

#[test]
fn lint_file_reads_and_runs_rules() {
    let dir = tempfile::tempdir().unwrap();
    let claude_md = dir.path().join("CLAUDE.md");
    std::fs::write(&claude_md, "# My Project\n\nShort and sweet.\n").unwrap();
    let report = lint_file(&claude_md).unwrap();
    // A short, well-structured file should produce no errors
    assert!(report.passed());
}
