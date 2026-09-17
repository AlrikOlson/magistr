use super::*;
use crate::scaffold::{AdminOperation, Ownership};

#[test]
fn parse_action_single_chars() {
    assert_eq!(parse_action("a"), Some(ApprovalAction::Apply));
    assert_eq!(parse_action("s"), Some(ApprovalAction::Skip));
    assert_eq!(parse_action("v"), Some(ApprovalAction::View));
    assert_eq!(parse_action("A"), Some(ApprovalAction::ApplyAll));
    assert_eq!(parse_action("q"), Some(ApprovalAction::Quit));
}

#[test]
fn parse_action_full_words() {
    assert_eq!(parse_action("apply"), Some(ApprovalAction::Apply));
    assert_eq!(parse_action("skip"), Some(ApprovalAction::Skip));
    assert_eq!(parse_action("view"), Some(ApprovalAction::View));
    assert_eq!(parse_action("quit"), Some(ApprovalAction::Quit));
}

#[test]
fn parse_action_trims_whitespace() {
    assert_eq!(parse_action("  a  "), Some(ApprovalAction::Apply));
    assert_eq!(parse_action("s\n"), Some(ApprovalAction::Skip));
}

#[test]
fn parse_action_unknown_returns_none() {
    assert_eq!(parse_action("x"), None);
    assert_eq!(parse_action(""), None);
    assert_eq!(parse_action("apply-all"), None);
}

#[test]
fn format_file_header_create() {
    let f = PlannedChange {
        ownership: Ownership::Unclaimed,
        ownership_context: None,
        path: ".magistr/project.json".to_string(),
        action: FileAction::Create,
        new_content: "{}".to_string(),
        existing_content: None,
        diff: None,
    };
    let header = format_file_header(0, 5, &f);
    assert!(header.contains("[1/5]"));
    assert!(header.contains(".magistr/project.json"));
    assert!(header.contains("[CREATE]"));
}

#[test]
fn format_file_header_overwrite() {
    let f = PlannedChange {
        ownership: Ownership::Unclaimed,
        ownership_context: None,
        path: ".claude/settings.json".to_string(),
        action: FileAction::Overwrite,
        new_content: "new".to_string(),
        existing_content: Some("old".to_string()),
        diff: Some("- old\n+ new".to_string()),
    };
    let header = format_file_header(2, 10, &f);
    assert!(header.contains("[3/10]"));
    assert!(header.contains("[OVERWRITE]"));
}

#[test]
fn print_file_detail_shows_content() {
    let f = PlannedChange {
        ownership: Ownership::Unclaimed,
        ownership_context: None,
        path: "test.txt".to_string(),
        action: FileAction::Create,
        new_content: "line1\nline2".to_string(),
        existing_content: None,
        diff: None,
    };

    let mut output = Vec::new();
    print_file_detail(&mut output, &f).unwrap();
    let s = String::from_utf8(output).unwrap();

    assert!(s.contains("New content"));
    assert!(s.contains("line1"));
    assert!(s.contains("line2"));
    // No existing content section
    assert!(!s.contains("Existing content"));
}

#[test]
fn print_file_detail_shows_existing_and_diff() {
    let f = PlannedChange {
        ownership: Ownership::Unclaimed,
        ownership_context: None,
        path: "test.txt".to_string(),
        action: FileAction::Overwrite,
        new_content: "new".to_string(),
        existing_content: Some("old".to_string()),
        diff: Some("--- a/test.txt\n+++ b/test.txt\n- old\n+ new".to_string()),
    };

    let mut output = Vec::new();
    print_file_detail(&mut output, &f).unwrap();
    let s = String::from_utf8(output).unwrap();

    assert!(s.contains("Existing content"));
    assert!(s.contains("old"));
    assert!(s.contains("New content"));
    assert!(s.contains("new"));
    assert!(s.contains("Diff"));
}

#[test]
fn interactive_approve_apply_single() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![PlannedChange {
            ownership: Ownership::Unclaimed,
            ownership_context: None,
            path: ".magistr/project.json".to_string(),
            action: FileAction::Create,
            new_content: "{}".to_string(),
            existing_content: None,
            diff: None,
        }],
    };

    let mut input = std::io::Cursor::new(b"a\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    assert_eq!(result.approved_indices, vec![0]);
    assert!(!result.quit_early);
}

#[test]
fn interactive_approve_skip_single() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![PlannedChange {
            ownership: Ownership::Unclaimed,
            ownership_context: None,
            path: ".magistr/project.json".to_string(),
            action: FileAction::Create,
            new_content: "{}".to_string(),
            existing_content: None,
            diff: None,
        }],
    };

    let mut input = std::io::Cursor::new(b"s\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    assert!(result.approved_indices.is_empty());
    assert!(!result.quit_early);
}

#[test]
fn interactive_approve_quit_skips_remaining() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "file1".to_string(),
                action: FileAction::Create,
                new_content: "a".to_string(),
                existing_content: None,
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "file2".to_string(),
                action: FileAction::Create,
                new_content: "b".to_string(),
                existing_content: None,
                diff: None,
            },
        ],
    };

    let mut input = std::io::Cursor::new(b"q\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    assert!(result.approved_indices.is_empty());
    assert!(result.quit_early);
}

#[test]
fn interactive_approve_apply_all() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "file1".to_string(),
                action: FileAction::Create,
                new_content: "a".to_string(),
                existing_content: None,
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "file2".to_string(),
                action: FileAction::Overwrite,
                new_content: "b".to_string(),
                existing_content: Some("old".to_string()),
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "file3".to_string(),
                action: FileAction::Unchanged,
                new_content: "c".to_string(),
                existing_content: Some("c".to_string()),
                diff: None,
            },
        ],
    };

    let mut input = std::io::Cursor::new(b"A\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    // file1 (Create) and file2 (Overwrite) approved, file3 (Unchanged) skipped
    assert_eq!(result.approved_indices, vec![0, 1]);
    assert!(!result.quit_early);
}

#[test]
fn interactive_approve_skips_unchanged_automatically() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "unchanged".to_string(),
                action: FileAction::Unchanged,
                new_content: "same".to_string(),
                existing_content: Some("same".to_string()),
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "new".to_string(),
                action: FileAction::Create,
                new_content: "content".to_string(),
                existing_content: None,
                diff: None,
            },
        ],
    };

    let mut input = std::io::Cursor::new(b"a\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    // Only the Create file was prompted and approved
    assert_eq!(result.approved_indices, vec![1]);
}

#[test]
fn interactive_approve_skips_preserved_automatically() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "CLAUDE.md".to_string(),
                action: FileAction::Preserved,
                new_content: "generated".to_string(),
                existing_content: Some("user content".to_string()),
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "new.txt".to_string(),
                action: FileAction::Create,
                new_content: "hi".to_string(),
                existing_content: None,
                diff: None,
            },
        ],
    };

    let mut input = std::io::Cursor::new(b"s\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    // Preserved was auto-skipped, only new.txt was prompted (and skipped by user)
    assert!(result.approved_indices.is_empty());
}

#[test]
fn interactive_approve_view_then_apply() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![PlannedChange {
            ownership: Ownership::Unclaimed,
            ownership_context: None,
            path: "test.txt".to_string(),
            action: FileAction::Create,
            new_content: "content".to_string(),
            existing_content: None,
            diff: None,
        }],
    };

    // User types 'v' to view, then 'a' to apply
    let mut input = std::io::Cursor::new(b"v\na\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    assert_eq!(result.approved_indices, vec![0]);
    let output_str = String::from_utf8(output).unwrap();
    assert!(output_str.contains("New content"));
}

#[test]
fn interactive_approve_mixed_actions() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "apply-me".to_string(),
                action: FileAction::Create,
                new_content: "a".to_string(),
                existing_content: None,
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "skip-me".to_string(),
                action: FileAction::Create,
                new_content: "b".to_string(),
                existing_content: None,
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "also-apply".to_string(),
                action: FileAction::Overwrite,
                new_content: "c".to_string(),
                existing_content: Some("old".to_string()),
                diff: None,
            },
        ],
    };

    let mut input = std::io::Cursor::new(b"a\ns\na\n" as &[u8]);
    let mut output = Vec::new();

    let result = interactive_approve(&preview, &mut input, &mut output).unwrap();

    assert_eq!(result.approved_indices, vec![0, 2]);
    assert!(!result.quit_early);
}

#[test]
fn approval_result_approved_files() {
    let preview = ChangePlan {
        operation: AdminOperation::Setup,
        files: vec![
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "first".to_string(),
                action: FileAction::Create,
                new_content: "a".to_string(),
                existing_content: None,
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "second".to_string(),
                action: FileAction::Create,
                new_content: "b".to_string(),
                existing_content: None,
                diff: None,
            },
            PlannedChange {
                ownership: Ownership::Unclaimed,
                ownership_context: None,
                path: "third".to_string(),
                action: FileAction::Create,
                new_content: "c".to_string(),
                existing_content: None,
                diff: None,
            },
        ],
    };

    let result = ApprovalResult {
        approved_indices: vec![0, 2],
        quit_early: false,
    };

    let files = result.approved_files(&preview);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "first");
    assert_eq!(files[1].path, "third");
}
