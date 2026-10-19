use super::*;
use crate::error::{MagistrError, Result};
use crate::scaffold::vars;
use std::path::Path;

/// Write or refresh the section of `<dir>/CLAUDE.md`, creating the file when
/// there is none; answers whether the file changed.
fn upsert_claude_md_section(dir: &Path, vars: &HashMap<String, String>) -> Result<bool> {
    let path = dir.join("CLAUDE.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let next = with_claude_md_section(&existing, vars)?;
    if next == existing {
        return Ok(false);
    }
    std::fs::write(&path, next).map_err(|e| MagistrError::io(&path, e))?;
    Ok(true)
}

fn rust_vars() -> HashMap<String, String> {
    vars([
        ("language", "rust"),
        ("check_cmd", "cargo check --all-targets"),
        ("test_cmd", "cargo nextest run"),
        ("lint_cmd", "cargo clippy -- -D warnings"),
    ])
}

#[test]
fn section_is_bounded_by_the_shared_markers() {
    let section = build_section(&rust_vars());
    assert!(section.starts_with(CLAUDE_MD_MARKER));
    assert!(section.trim_end().ends_with(CLAUDE_MD_END_MARKER));
    assert!(section.contains("# magistr"));
    for family in ["think_*", "ship_*", "roadmap_*", "signal_*"] {
        assert!(section.contains(family), "names the {family} family");
    }
}

#[test]
fn section_lists_the_gate_commands_as_verification() {
    let section = build_section(&rust_vars());
    assert!(section.contains("This is a Rust project."));
    assert!(section.contains("- `cargo check --all-targets`"));
    assert!(section.contains("- `cargo nextest run`"));
    assert!(section.contains("- `cargo clippy -- -D warnings`"));
}

#[test]
fn section_omits_verification_without_gate_commands() {
    let section = build_section(&HashMap::new());
    assert!(!section.contains("## Verification"));
    assert!(section.trim_end().ends_with(CLAUDE_MD_END_MARKER));
}

#[test]
fn upsert_creates_claude_md_when_absent() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(upsert_claude_md_section(tmp.path(), &rust_vars()).unwrap());
    let text = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert_eq!(text, build_section(&rust_vars()));
}

#[test]
fn upsert_appends_after_user_content_and_keeps_it() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("CLAUDE.md"),
        "# My project\n\nHand-written.\n",
    )
    .unwrap();
    assert!(upsert_claude_md_section(tmp.path(), &rust_vars()).unwrap());
    let text = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert!(text.starts_with("# My project\n\nHand-written.\n\n<!-- magistr -->"));
    assert_eq!(text.matches(CLAUDE_MD_MARKER).count(), 1);
}

#[test]
fn upsert_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), "# My project\n").unwrap();
    assert!(upsert_claude_md_section(tmp.path(), &rust_vars()).unwrap());
    let once = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert!(!upsert_claude_md_section(tmp.path(), &rust_vars()).unwrap());
    let twice = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn upsert_refreshes_between_markers_and_keeps_both_sides() {
    let tmp = tempfile::TempDir::new().unwrap();
    let stale = format!(
        "# Before\n\n{CLAUDE_MD_MARKER}\n# magistr\n\nold body\n{CLAUDE_MD_END_MARKER}\n\n# After\n\nkept.\n"
    );
    std::fs::write(tmp.path().join("CLAUDE.md"), &stale).unwrap();
    assert!(upsert_claude_md_section(tmp.path(), &rust_vars()).unwrap());
    let text = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert!(text.starts_with("# Before\n\n<!-- magistr -->"));
    assert!(text.ends_with("<!-- /magistr -->\n\n# After\n\nkept.\n"));
    assert!(!text.contains("old body"));
    assert!(text.contains("cargo nextest run"));
    assert_eq!(
        text.matches(magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE)
            .count(),
        1,
    );
}

#[test]
fn an_unclosed_section_is_refused_without_changing_user_text() {
    let tmp = tempfile::TempDir::new().unwrap();
    let broken = format!("{CLAUDE_MD_MARKER}\n# User text\n");
    std::fs::write(tmp.path().join("CLAUDE.md"), &broken).unwrap();
    assert!(upsert_claude_md_section(tmp.path(), &rust_vars()).is_err());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
        broken
    );
}

#[test]
fn the_section_names_the_public_catalog_and_native_owners() {
    let section = build_section(&rust_vars());
    for name in [
        "magistr mcp --dir .",
        "discover",
        "execute",
        "plan_work",
        "check_work",
        "One workflow owner",
    ] {
        assert!(section.contains(name), "{name}");
    }
}
