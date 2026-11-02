//! Mirrors `tests/formatter.test.ts`.

use std::collections::BTreeMap;

use magistr_workflow::decision::domain::{
    Log, NextAction, Note, StructuredAction, Thread, ThreadStatus,
};
use magistr_workflow::decision::formatter::Formatter;
use serde_json::json;

fn base_note() -> Note {
    Note {
        note: 1,
        estimated_total: 3,
        purpose: "analysis".into(),
        context: "Test context".into(),
        thought: "Test thought".into(),
        outcome: "Test outcome".into(),
        next_action: NextAction::Text("Test next action".into()),
        rationale: "Test rationale".into(),
        confidence: None,
        uncertainty_notes: None,
        revises_note: None,
        revision_reason: None,
        revised_by: None,
        is_final_note: None,
        thread_from: None,
        thread_id: None,
        thread_name: None,
        tools_used: None,
        dependencies: None,
        timestamp: None,
        duration_ms: None,
        log_id: None,
        pinned: None,
        cwd: None,
        run_ref: None,
        record_id: None,
    }
}

#[test]
fn console_basic_no_color() {
    let f = Formatter::new(false);
    let out = f.format_note_console(&base_note());
    assert!(out.contains("[Note 1/3] ANALYSIS"));
    assert!(out.contains("Context: Test context"));
    assert!(out.contains("Thought: Test thought"));
    assert!(out.contains("Outcome: Test outcome"));
    assert!(out.contains("Next: Test next action - Test rationale"));
}

#[test]
fn console_includes_confidence_percentage() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.confidence = Some(0.85);
    let out = f.format_note_console(&note);
    assert!(out.contains("[85%]"), "missing 85% in: {out}");
}

#[test]
fn console_shows_revision_info() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.revises_note = Some(2);
    note.revision_reason = Some("Found error in note 2".into());
    let out = f.format_note_console(&note);
    assert!(out.contains("Revises #2"));
    assert!(out.contains("Revision Reason: Found error in note 2"));
}

#[test]
fn console_shows_thread_info() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.thread_from = Some(1);
    note.thread_name = Some("Alternative approach".into());
    let out = f.format_note_console(&note);
    assert!(out.contains("Thread from #1"));
    assert!(out.contains("(Alternative approach)"));
}

#[test]
fn console_shows_uncertainty() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.uncertainty_notes = Some("Not sure about this approach".into());
    let out = f.format_note_console(&note);
    assert!(out.contains("Uncertainty: Not sure about this approach"));
}

#[test]
fn console_shows_tools_used() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.tools_used = Some(vec!["Read".into(), "Grep".into(), "Edit".into()]);
    let out = f.format_note_console(&note);
    assert!(out.contains("Tools Used: Read, Grep, Edit"));
}

#[test]
fn console_shows_dependencies() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.dependencies = Some(vec![1u32.into(), 2u32.into(), 3u32.into()]);
    let out = f.format_note_console(&note);
    assert!(out.contains("Depends On: Notes 1, 2, 3"));
}

#[test]
fn console_formats_structured_action() {
    let f = Formatter::new(false);
    let mut note = base_note();
    let mut params = BTreeMap::new();
    params.insert("path".into(), json!("/config.ts"));
    note.next_action = NextAction::Structured(StructuredAction {
        tool: Some("Read".into()),
        action: "Read the config file".into(),
        parameters: Some(params),
        expected_output: None,
    });
    let out = f.format_note_console(&note);
    assert!(out.contains("[Read] Read the config file"));
    assert!(out.contains("{\"path\":\"/config.ts\"}"));
}

#[test]
fn console_handles_all_purpose_types() {
    let f = Formatter::new(false);
    for purpose in [
        "analysis",
        "action",
        "reflection",
        "decision",
        "summary",
        "validation",
        "exploration",
        "hypothesis",
        "correction",
        "planning",
    ] {
        let mut note = base_note();
        note.purpose = purpose.into();
        let out = f.format_note_console(&note);
        assert!(
            out.contains(&format!("[Note 1/3] {}", purpose.to_uppercase())),
            "missing uppercase {purpose} in: {out}"
        );
    }
}

#[test]
fn console_handles_custom_purpose() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.purpose = "custom-purpose".into();
    let out = f.format_note_console(&note);
    assert!(out.contains("[Note 1/3] CUSTOM-PURPOSE"));
}

#[test]
fn markdown_basic() {
    let f = Formatter::new(false);
    let out = f.format_note_markdown(&base_note());
    assert!(out.contains("### Note 1/3: ANALYSIS"));
    assert!(out.contains("**Context:** Test context"));
    assert!(out.contains("**Thought:** Test thought"));
    assert!(out.contains("**Outcome:** Test outcome"));
    assert!(out.contains("**Next Action:** Test next action"));
    assert!(out.contains("*Rationale:* Test rationale"));
    assert!(out.contains("---"));
}

#[test]
fn markdown_confidence_badge() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.confidence = Some(0.75);
    let out = f.format_note_markdown(&note);
    assert!(out.contains("![Confidence]"));
    assert!(out.contains("75%"));
}

#[test]
fn markdown_revises_badge() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.revises_note = Some(2);
    let out = f.format_note_markdown(&note);
    assert!(out.contains("![Revises]"));
    assert!(out.contains("note%202"));
}

#[test]
fn markdown_thread_badge() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.thread_from = Some(3);
    let out = f.format_note_markdown(&note);
    assert!(out.contains("![Thread]"));
    assert!(out.contains("from%203"));
}

#[test]
fn markdown_uncertainty_blockquote() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.uncertainty_notes = Some("High uncertainty here".into());
    let out = f.format_note_markdown(&note);
    assert!(out.contains("> ⚠️ **Uncertainty:** High uncertainty here"));
}

#[test]
fn markdown_revision_reason_blockquote() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.revises_note = Some(1);
    note.revision_reason = Some("Previous analysis was incorrect".into());
    let out = f.format_note_markdown(&note);
    assert!(out.contains("> 🔄 **Revision Reason:** Previous analysis was incorrect"));
}

#[test]
fn markdown_tools_used() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.tools_used = Some(vec!["Bash".into(), "Read".into()]);
    let out = f.format_note_markdown(&note);
    assert!(out.contains("**Tools Used:** Bash, Read"));
}

#[test]
fn json_is_valid() {
    let f = Formatter::new(false);
    let out = f.format_note_json(&base_note());
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["note"], 1);
    assert_eq!(parsed["estimated_total"], 3);
    assert_eq!(parsed["purpose"], "analysis");
}

#[test]
fn json_includes_optional_fields() {
    let f = Formatter::new(false);
    let mut note = base_note();
    note.confidence = Some(0.9);
    note.revises_note = Some(1);
    note.thread_from = Some(2);
    note.tools_used = Some(vec!["Test".into()]);
    let out = f.format_note_json(&note);
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["confidence"], 0.9);
    assert_eq!(parsed["revises_note"], 1);
    assert_eq!(parsed["thread_from"], 2);
    assert_eq!(parsed["tools_used"], json!(["Test"]));
}

#[test]
fn thread_tree_shows_main_notes() {
    let f = Formatter::new(false);
    let mut s1 = base_note();
    s1.purpose = "analysis".into();
    let mut s2 = base_note();
    s2.note = 2;
    s2.purpose = "action".into();
    let history = Log {
        notes: vec![s1, s2],
        threads: None,
        completed: false,
        log_id: None,
        created_at: None,
        updated_at: None,
        metadata: None,
    };
    let out = f.format_thread_tree(&history);
    assert!(out.contains("Thread Structure:"));
    assert!(out.contains("Main:"));
    assert!(out.contains("Note 1: analysis"));
    assert!(out.contains("Note 2: action"));
}

#[test]
fn thread_tree_shows_threads() {
    let f = Formatter::new(false);
    let mut thread_note = base_note();
    thread_note.note = 2;
    thread_note.purpose = "exploration".into();
    thread_note.thread_id = Some("thread-1".into());
    let history = Log {
        notes: vec![base_note(), thread_note.clone()],
        threads: Some(vec![Thread {
            id: "thread-1".into(),
            name: "Alt approach".into(),
            from_note: 1,
            notes: vec![thread_note],
            status: ThreadStatus::Open,
            created_at: "".into(),
            depth: 1,
            merged_into: None,
        }]),
        completed: false,
        log_id: None,
        created_at: None,
        updated_at: None,
        metadata: None,
    };
    let out = f.format_thread_tree(&history);
    assert!(out.contains("Thread: Alt approach"));
}

#[test]
fn color_disabled_emits_no_ansi() {
    let f = Formatter::new(false);
    let out = f.format_note_console(&base_note());
    assert!(!out.contains('\x1b'), "expected no ANSI codes, got: {out}");
    assert!(out.contains("[Note 1/3] ANALYSIS"));
}

#[test]
fn color_enabled_still_carries_content() {
    let f_color = Formatter::new(true);
    let f_no_color = Formatter::new(false);
    let s = base_note();
    let c = f_color.format_note_console(&s);
    let n = f_no_color.format_note_console(&s);
    assert!(c.contains("[Note 1/3]"));
    assert!(n.contains("[Note 1/3]"));
    assert!(!n.contains('\x1b'));
}
