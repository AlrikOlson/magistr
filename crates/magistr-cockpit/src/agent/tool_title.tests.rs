use super::*;
use crate::agent::{ToolStatus, ToolVerb};
use crate::turn::{ConversationCard, Turn};

#[test]
fn known_tool_names_keep_meaningful_descriptions_without_internal_names() {
    for (input, expected) in [
        ("mcp__magistr__ship_complete", "Finish a work item"),
        ("mcp__magistr__ship_start", "Start a work item"),
        (
            "mcp__magistr__roadmap_update_chunk: Split recovery work",
            "Update a roadmap item: Split recovery work",
        ),
        ("mcp__magistr__think_record_step", "Record reasoning"),
        ("mcp__magistr__magistr_set_phase", "Update work progress"),
        (
            "mcp__workbench_mcp__edit: Fix the retry limit",
            "Update files: Fix the retry limit",
        ),
        ("Bash: Check formatting", "Run a command: Check formatting"),
        (
            "mcp__unknown__opaque_call: Review retry settings",
            "Use a tool: Review retry settings",
        ),
        ("opaque_call", "Use a tool"),
        ("Review retry settings", "Review retry settings"),
    ] {
        assert_eq!(ToolTitle::display(input), expected);
    }
}

#[test]
fn normal_titles_are_shared_by_plain_rows_cards_updates_and_edit_headers() {
    let mut turn = Turn::new(100);
    turn.push_tool(
        "private-id",
        "mcp__magistr__ship_complete",
        ToolVerb::Tool,
        ToolStatus::InProgress,
    );
    assert_eq!(turn.lines()[0].text(), "Finish a work item");
    assert_eq!(
        ConversationCard::of(&turn, &turn.lines()[0]).unwrap().title,
        "Finish a work item"
    );
    turn.update_tool(
        "private-id",
        Some("mcp__magistr__ship_record: Save the outcome"),
        None,
        None,
    );
    assert_eq!(
        turn.lines()[0].text(),
        "Save a work update: Save the outcome"
    );
    turn.set_tool_title(
        "private-id",
        "mcp__workbench_mcp__edit: Update retry limits",
    );
    assert_eq!(
        turn.lines()[0].title(),
        Some("Update files: Update retry limits")
    );
}
