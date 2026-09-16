use super::*;
use crate::scaffold::vars;

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
    assert!(section.contains("# think-and-ship"));
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
    assert!(text.starts_with("# My project\n\nHand-written.\n\n<!-- think-and-ship -->"));
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
        "# Before\n\n{CLAUDE_MD_MARKER}\n# think-and-ship\n\nold body\n{CLAUDE_MD_END_MARKER}\n\n# After\n\nkept.\n"
    );
    std::fs::write(tmp.path().join("CLAUDE.md"), &stale).unwrap();
    assert!(upsert_claude_md_section(tmp.path(), &rust_vars()).unwrap());
    let text = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert!(text.starts_with("# Before\n\n<!-- think-and-ship -->"));
    assert!(text.ends_with("<!-- /think-and-ship -->\n\n# After\n\nkept.\n"));
    assert!(!text.contains("old body"));
    assert!(text.contains("cargo nextest run"));
    assert_eq!(
        text.matches(magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE)
            .count(),
        1,
    );
}

#[test]
fn upsert_bounds_a_legacy_unterminated_section_at_the_next_heading() {
    let tmp = tempfile::TempDir::new().unwrap();
    let legacy = format!("{CLAUDE_MD_MARKER}\n# think-and-ship\n\nold body\n\n# Mine\n\nkept.\n");
    std::fs::write(tmp.path().join("CLAUDE.md"), &legacy).unwrap();
    assert!(upsert_claude_md_section(tmp.path(), &rust_vars()).unwrap());
    let text = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
    assert!(!text.contains("old body"));
    assert!(text.contains("<!-- /think-and-ship -->\n# Mine\n\nkept.\n"));
    assert_eq!(text.matches("# think-and-ship").count(), 1);
}

#[test]
fn the_section_names_the_magistr_server_and_forbids_a_second_process() {
    let section = build_section(&rust_vars());
    assert!(
        section.contains("in-process by the `magistr` MCP server"),
        "the one process over the store is named: {section}"
    );
    assert!(
        section.contains("Do not also configure a standalone `think-and-ship serve`"),
        "{section}"
    );
    assert!(
        !section.contains("One MCP server is configured: **think-and-ship**"),
        "the old line named the wrong process"
    );
}
