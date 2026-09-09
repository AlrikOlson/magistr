use super::*;
use crate::theme::Icons;

fn prose(text: &str, columns: u16) -> Vec<Row> {
    wrap_line(
        &LineKind::Prose(Voice::Agent),
        text,
        false,
        None,
        &Icons::UNICODE,
        columns,
    )
}

fn texts(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|r| r.text.as_str()).collect()
}

#[test]
fn plain_prose_word_wraps_in_its_voice() {
    let rows = wrap_line(
        &LineKind::Prose(Voice::Thought),
        "one two three four",
        false,
        None,
        &Icons::UNICODE,
        9,
    );
    assert!(
        rows.iter()
            .all(|r| r.kind == RowKind::Prose(Voice::Thought))
    );
    assert_eq!(texts(&rows), ["one two", "three", "four"]);
}

#[test]
fn an_empty_line_is_one_empty_row_and_no_columns_is_no_row() {
    assert_eq!(
        prose("", 20),
        vec![Row::new(RowKind::Prose(Voice::Agent), "")]
    );
    assert!(prose("anything", 0).is_empty());
    assert!(wrap_line(&LineKind::Note, "", false, None, &Icons::UNICODE, 0).is_empty());
}

#[test]
fn a_heading_loses_its_marker_and_reads_as_a_heading() {
    let rows = prose("## Plan of record", 40);
    assert_eq!(rows, vec![Row::new(RowKind::Heading, "Plan of record")]);
    assert_eq!(prose("#", 40)[0].kind, RowKind::Heading);
    assert_eq!(
        prose("#hashtag", 40)[0].kind,
        RowKind::Prose(Voice::Agent),
        "no space, no heading"
    );
    assert_eq!(
        prose("####### seven", 40)[0].kind,
        RowKind::Prose(Voice::Agent)
    );
}

#[test]
fn a_fence_line_and_a_fenced_line_are_code_broken_by_columns() {
    let fence = prose("```rust", 40);
    assert_eq!(
        fence,
        vec![Row::new(
            RowKind::Fence,
            format!("── rust {}", "─".repeat(32))
        )]
    );
    let inside = wrap_line(
        &LineKind::Prose(Voice::Agent),
        "    let long_name = call(a, b);",
        true,
        None,
        &Icons::UNICODE,
        12,
    );
    assert!(inside.iter().all(|r| r.kind == RowKind::Code));
    assert_eq!(
        texts(&inside),
        ["    let long", "_name = call", "(a, b);"],
        "hard-wrapped: the indentation survives"
    );
    let fenced_heading = wrap_line(
        &LineKind::Prose(Voice::Agent),
        "# not a heading",
        true,
        None,
        &Icons::UNICODE,
        40,
    );
    assert_eq!(fenced_heading[0].kind, RowKind::Code);
}

#[test]
fn list_items_get_a_marker_and_indented_continuations() {
    let rows = prose("- alpha beta gamma delta", 12);
    assert!(rows.iter().all(|r| r.kind == RowKind::Bullet));
    assert_eq!(texts(&rows), ["• alpha beta", "  gamma", "  delta"]);
    let numbered = prose("12. twelve things", 30);
    assert_eq!(texts(&numbered), ["12. twelve things"]);
    assert_eq!(numbered[0].kind, RowKind::Bullet);
    let star = prose("  * nested", 30);
    assert_eq!(texts(&star), ["• nested"]);
    assert_eq!(
        prose("-dash", 30)[0].kind,
        RowKind::Prose(Voice::Agent),
        "no space, no item"
    );
    assert_eq!(
        prose("- x", 2)[0].kind,
        RowKind::Prose(Voice::Agent),
        "too narrow for a marker"
    );
}

#[test]
fn a_tool_line_is_one_row_with_its_status_glyph_cut_to_the_width() {
    let kind = |status| LineKind::Tool {
        id: "c1".to_owned(),
        status,
        verb: ToolVerb::Run,
        duration_ms: None,
        expanded: false,
    };
    assert_eq!(
        wrap_line(
            &kind(ToolStatus::InProgress),
            "cargo test",
            false,
            None,
            &Icons::UNICODE,
            40
        ),
        vec![Row::new(
            RowKind::Tool(ToolStatus::InProgress),
            "● run    cargo test"
        )]
    );
    assert_eq!(
        wrap_line(
            &kind(ToolStatus::Pending),
            "x",
            false,
            None,
            &Icons::UNICODE,
            40
        )[0]
        .text,
        "○ run    x"
    );
    assert_eq!(
        wrap_line(
            &kind(ToolStatus::Completed),
            "x",
            false,
            None,
            &Icons::UNICODE,
            40
        )[0]
        .text,
        "✓ run    x"
    );
    assert_eq!(
        wrap_line(
            &kind(ToolStatus::Failed),
            "x",
            false,
            None,
            &Icons::UNICODE,
            40
        )[0]
        .text,
        "✗ run    x"
    );
    let long = wrap_line(
        &kind(ToolStatus::Completed),
        "read a very long file name",
        false,
        None,
        &Icons::UNICODE,
        12,
    );
    assert_eq!(long.len(), 1);
    assert_eq!(long[0].text, "✓ run    re…");
    assert_eq!(crate::render::width(&long[0].text), 12);
}

#[test]
fn a_finished_tool_line_shows_its_duration() {
    let kind = LineKind::Tool {
        id: "c1".to_owned(),
        status: ToolStatus::Completed,
        verb: ToolVerb::Run,
        duration_ms: Some(1234),
        expanded: false,
    };
    assert_eq!(
        wrap_line(&kind, "cargo test", false, None, &Icons::UNICODE, 40)[0].text,
        "✓ run    cargo test · 1.2s"
    );
}

#[test]
fn a_fence_line_is_a_rule_naming_the_language() {
    let open = wrap_line(
        &LineKind::Prose(Voice::Agent),
        "```rust",
        false,
        None,
        &Icons::UNICODE,
        16,
    );
    assert_eq!(open, vec![Row::new(RowKind::Fence, "── rust ────────")]);
    let close = wrap_line(
        &LineKind::Prose(Voice::Agent),
        "```",
        true,
        None,
        &Icons::UNICODE,
        16,
    );
    assert_eq!(close, vec![Row::new(RowKind::Fence, "────────────────")]);
    let bare = wrap_line(
        &LineKind::Prose(Voice::Agent),
        "```",
        false,
        None,
        &Icons::UNICODE,
        8,
    );
    assert_eq!(bare[0].text, "────────", "an unnamed fence is a plain rule");
}

#[test]
fn notes_and_errors_word_wrap_in_their_kind_and_a_carriage_return_is_dropped() {
    let note = wrap_line(
        &LineKind::Note,
        "turn ended: max tokens\r",
        false,
        None,
        &Icons::UNICODE,
        12,
    );
    assert_eq!(texts(&note), ["turn ended:", "max tokens"]);
    assert!(note.iter().all(|r| r.kind == RowKind::Note));
    let error = wrap_line(&LineKind::Error, "boom", false, None, &Icons::UNICODE, 12);
    assert_eq!(error, vec![Row::new(RowKind::Error, "boom")]);
}

#[test]
fn wide_graphemes_count_their_columns_in_code_and_cuts() {
    let code = wrap_line(
        &LineKind::Prose(Voice::Agent),
        "日本語テキスト",
        true,
        None,
        &Icons::UNICODE,
        6,
    );
    assert_eq!(texts(&code), ["日本語", "テキス", "ト"]);
    let tool = LineKind::Tool {
        id: "t".to_owned(),
        status: ToolStatus::Completed,
        verb: ToolVerb::Tool,
        duration_ms: None,
        expanded: false,
    };
    let cut = wrap_line(&tool, "日本語テキスト", false, None, &Icons::UNICODE, 7);
    assert_eq!(cut[0].text, "✓ tool…");
}
