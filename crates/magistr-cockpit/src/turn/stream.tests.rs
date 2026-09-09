use super::*;
use crate::theme::ThemeMode;
use crate::turn::MAX_IMAGE_ROWS;
use crate::turn::{RowKind, SpanStyle};

fn texts(turn: &Turn) -> Vec<&str> {
    turn.rows().iter().map(|r| r.text.as_str()).collect()
}

fn kinds(turn: &Turn) -> Vec<RowKind> {
    turn.rows().iter().map(|r| r.kind).collect()
}

#[test]
fn deltas_append_to_the_open_line_and_re_wrap_only_its_rows() {
    let mut turn = Turn::new(10);
    turn.push_text(Voice::Agent, "hello ");
    assert_eq!(texts(&turn), ["hello"]);
    turn.push_text(Voice::Agent, "wor");
    assert_eq!(texts(&turn), ["hello wor"]);
    turn.push_text(Voice::Agent, "ld again");
    assert_eq!(texts(&turn), ["hello", "world", "again"]);
    assert_eq!(turn.lines().len(), 1);
    assert_eq!(turn.len(), 3);
    assert!(!turn.is_empty());
}

#[test]
fn newlines_close_lines_and_a_trailing_newline_opens_none() {
    let mut turn = Turn::new(20);
    turn.push_text(Voice::Agent, "one\ntwo\n");
    assert_eq!(texts(&turn), ["one", "two"]);
    assert_eq!(turn.lines().len(), 2);
    turn.push_text(Voice::Agent, "three");
    assert_eq!(texts(&turn), ["one", "two", "three"]);
    assert_eq!(turn.lines().len(), 3);
    turn.push_text(Voice::Agent, "");
    assert_eq!(turn.lines().len(), 3, "an empty delta opens nothing");
}

#[test]
fn a_blank_line_between_paragraphs_is_one_empty_row() {
    let mut turn = Turn::new(20);
    turn.push_text(Voice::Agent, "para one\n\npara two");
    assert_eq!(texts(&turn), ["para one", "", "para two"]);
    let mut split = Turn::new(20);
    split.push_text(Voice::Agent, "para one\n");
    split.push_text(Voice::Agent, "\npara two");
    assert_eq!(texts(&split), ["para one", "", "para two"]);
}

#[test]
fn a_voice_change_closes_the_line() {
    let mut turn = Turn::new(20);
    turn.push_text(Voice::Thought, "thinking");
    turn.push_text(Voice::Agent, "saying");
    turn.push_text(Voice::Agent, " more");
    assert_eq!(texts(&turn), ["thinking", "saying more"]);
    assert_eq!(
        kinds(&turn),
        [RowKind::Prose(Voice::Thought), RowKind::Prose(Voice::Agent)]
    );
}

#[test]
fn the_fence_state_carries_across_lines_and_deltas() {
    let mut turn = Turn::new(30);
    turn.push_text(Voice::Agent, "# Title\n``");
    assert_eq!(
        kinds(&turn),
        [RowKind::Heading, RowKind::Prose(Voice::Agent)]
    );
    turn.push_text(Voice::Agent, "`\nlet x = 1;\n```\nafter");
    assert_eq!(
        kinds(&turn),
        [
            RowKind::Heading,
            RowKind::Fence,
            RowKind::Code,
            RowKind::Fence,
            RowKind::Prose(Voice::Agent)
        ]
    );
    let rule = "─".repeat(30);
    assert_eq!(
        texts(&turn),
        ["Title", &rule, "let x = 1;", &rule, "after"],
        "an unnamed fence draws a plain rule both sides"
    );
}

#[test]
fn a_tool_call_is_one_row_updated_in_place_by_id() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "Let me look.");
    turn.push_tool(
        "c1",
        "read src/main.rs",
        ToolVerb::Read,
        ToolStatus::InProgress,
    );
    turn.push_tool("c2", "cargo test", ToolVerb::Run, ToolStatus::Pending);
    turn.push_text(Voice::Agent, "Meanwhile");
    assert_eq!(
        texts(&turn),
        [
            "Let me look.",
            "● read   read src/main.rs",
            "○ run    cargo test",
            "Meanwhile"
        ]
    );
    assert!(turn.update_tool("c1", None, Some(ToolStatus::Completed), None));
    assert!(turn.update_tool(
        "c2",
        Some("cargo nextest run"),
        Some(ToolStatus::Failed),
        None
    ));
    assert!(!turn.update_tool("nope", None, Some(ToolStatus::Completed), None));
    assert_eq!(
        texts(&turn),
        [
            "Let me look.",
            "✓ read   read src/main.rs",
            "✗ run    cargo nextest run",
            "Meanwhile"
        ]
    );
    assert_eq!(turn.len(), 4);
    assert_eq!(turn.lines()[1].row_start(), 1);
    assert_eq!(turn.lines()[3].row_start(), 3);
}

#[test]
fn a_tool_calls_record_lives_beside_its_row_and_an_update_folds_into_it() {
    let mut turn = Turn::new(40);
    turn.push_tool("c1", "cargo test", ToolVerb::Run, ToolStatus::InProgress);
    turn.push_text(Voice::Agent, "after");
    let rows = turn.rows().to_vec();
    assert_eq!(turn.tool_record("c1"), Some(&ToolRecord::default()));
    assert_eq!(turn.tool_record("nope"), None);

    assert!(turn.set_tool_record(
        "c1",
        ToolRecord {
            input: Some(serde_json::json!({"command": "cargo test"})),
            ..ToolRecord::default()
        }
    ));
    assert!(turn.set_tool_record(
        "c1",
        ToolRecord {
            output: Some(serde_json::json!("ok")),
            text: vec!["test result: ok".to_owned()],
            ..ToolRecord::default()
        }
    ));
    assert!(!turn.set_tool_record("nope", ToolRecord::default()));
    let record = turn.tool_record("c1").expect("kept");
    assert_eq!(
        record.input,
        Some(serde_json::json!({"command": "cargo test"}))
    );
    assert_eq!(record.output, Some(serde_json::json!("ok")));
    assert_eq!(record.text, ["test result: ok".to_owned()]);
    assert_eq!(turn.lines()[0].record(), record);
    assert!(turn.lines()[1].record().is_empty());
    // Data, not rows: nothing moved or re-wrapped.
    assert_eq!(turn.rows(), rows.as_slice());

    // A record that re-kinds the call redraws its verb column only.
    assert!(turn.set_tool_record(
        "c1",
        ToolRecord {
            kind: Some(ToolVerb::Search),
            ..ToolRecord::default()
        }
    ));
    assert_eq!(texts(&turn), ["● search cargo test", "after"]);
    assert_eq!(turn.len(), rows.len());
}

#[test]
fn a_tool_call_closes_the_open_line_so_the_next_delta_starts_a_new_one() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "before");
    turn.push_tool("c1", "ls", ToolVerb::Run, ToolStatus::InProgress);
    turn.push_text(Voice::Agent, "after");
    assert_eq!(texts(&turn), ["before", "● run    ls", "after"]);
    assert_eq!(turn.lines().len(), 3);
}

#[test]
fn notes_and_errors_are_their_own_lines() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::User, "fix it");
    turn.push_note("turn ended: cancelled");
    turn.push_error("transport gone");
    assert_eq!(
        kinds(&turn),
        [RowKind::Prose(Voice::User), RowKind::Note, RowKind::Error]
    );
}

#[test]
fn rewrap_re_flows_every_line_and_keeps_the_open_line_open() {
    let mut turn = Turn::new(40);
    turn.push_text(
        Voice::Agent,
        "# Head\n```\ncode line here\n```\n- a list item that wraps\nopen",
    );
    let before = turn.len();
    turn.rewrap(8);
    assert_eq!(turn.width(), 8);
    assert!(turn.len() > before);
    assert_eq!(turn.rows()[0], Row::new(RowKind::Heading, "Head"));
    assert!(
        turn.rows()
            .iter()
            .filter(|r| matches!(r.kind, RowKind::Code | RowKind::Fence))
            .count()
            >= 4
    );
    let bullets: Vec<&str> = turn
        .rows()
        .iter()
        .filter(|r| r.kind == RowKind::Bullet)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(bullets[0], "• a list");
    assert!(bullets[1..].iter().all(|t| t.starts_with("  ")));
    let starts: Vec<usize> = turn.lines().iter().map(Line::row_start).collect();
    assert!(starts.windows(2).all(|w| w[0] < w[1]));
    turn.push_text(Voice::Agent, "ed");
    assert_eq!(turn.rows().last().map(|r| r.text.as_str()), Some("opened"));
    assert_eq!(turn.lines().len(), 6);
}

#[test]
fn rewrap_inside_an_open_fence_keeps_the_fence_open() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "```\nlet a = 1;");
    turn.rewrap(20);
    turn.push_text(Voice::Agent, "\nlet b = 2;");
    assert!(
        kinds(&turn)
            .iter()
            .all(|k| matches!(k, RowKind::Code | RowKind::Fence))
    );
    assert_eq!(
        texts(&turn),
        ["─".repeat(20).as_str(), "let a = 1;", "let b = 2;"]
    );
}

#[test]
fn ten_thousand_lines_wrap_and_rewrap() {
    let mut turn = Turn::new(78);
    for i in 0..10_000 {
        turn.push_text(Voice::Agent, &format!("line {i}\n"));
    }
    assert_eq!(turn.len(), 10_000);
    assert_eq!(turn.lines().len(), 10_000);
    turn.rewrap(4);
    assert!(turn.len() > 10_000);
    assert_eq!(turn.rows()[0].text, "line");
}

#[test]
fn a_closed_fenced_line_takes_syntect_spans_and_the_open_tail_stays_plain() {
    let mut turn = Turn::new(60);
    turn.push_text(
        Voice::Agent,
        "```rust\nfn main() { println!(\"hi\"); }\nlet open = 1",
    );
    let rows = turn.rows();
    assert_eq!(rows[0].text, format!("── rust {}", "─".repeat(52)));
    assert!(rows[0].spans.is_empty(), "the fence line renders plain");
    assert!(!rows[1].spans.is_empty(), "the closed line is highlighted");
    assert!(
        rows[2].spans.is_empty(),
        "the open tail is plain until it closes"
    );
    turn.push_text(Voice::Agent, ";\n");
    assert!(
        !turn.rows()[2].spans.is_empty(),
        "closing the line is the one highlight pass it gets"
    );
}

#[test]
fn inline_markdown_streamed_in_pieces_lands_as_spans() {
    let mut turn = Turn::new(60);
    turn.push_text(Voice::Agent, "some **bo");
    turn.push_text(Voice::Agent, "ld** and `code`\n");
    assert_eq!(turn.rows()[0].text, "some bold and code");
    let styles: Vec<_> = turn.rows()[0]
        .spans
        .iter()
        .map(|s| s.style.clone())
        .collect();
    assert_eq!(styles, vec![SpanStyle::Bold, SpanStyle::Code]);
}

#[test]
fn a_theme_mode_change_re_highlights_the_code() {
    let mut turn = Turn::new(60);
    turn.push_text(Voice::Agent, "```rust\nfn main() {}\n```\nafter");
    let dark = turn.rows()[1].spans.clone();
    assert!(!dark.is_empty());
    turn.set_mode(ThemeMode::Light);
    let light = turn.rows()[1].spans.clone();
    assert_ne!(dark, light, "the two themes colour the keyword differently");
    assert_eq!(
        turn.rows().last().map(|r| r.text.as_str()),
        Some("after"),
        "the open tail survives the re-wrap"
    );
}

#[test]
fn a_tool_call_expands_to_its_full_title_and_collapses_back() {
    let mut turn = Turn::new(20);
    turn.push_text(Voice::Agent, "before\n");
    turn.push_tool(
        "c1",
        "cargo nextest run -p magistr-tui",
        ToolVerb::Run,
        ToolStatus::InProgress,
    );
    turn.push_text(Voice::Agent, "after\n");
    assert_eq!(
        texts(&turn),
        ["before", "● run    cargo next…", "after"],
        "collapsed: one cut row"
    );
    assert!(turn.toggle_at_row(1));
    let rows = texts(&turn);
    assert_eq!(rows[1], "● run    cargo next…", "the head row stays");
    assert!(rows[2].starts_with("  cargo nextest"), "{rows:?}");
    assert_eq!(rows.last(), Some(&"after"), "later lines re-anchor");
    assert!(turn.len() > 3);
    assert_eq!(kinds(&turn)[2], RowKind::Code, "detail rows read as code");
    // A detail row belongs to the same call: toggling there collapses.
    assert!(turn.toggle_at_row(2));
    assert_eq!(texts(&turn), ["before", "● run    cargo next…", "after"]);
    assert!(!turn.toggle_at_row(0), "prose rows have no toggle");
}

#[test]
fn the_last_tool_toggles_by_key_and_updates_while_expanded_rewrap() {
    let mut turn = Turn::new(30);
    assert!(!turn.toggle_last(), "no tool yet");
    turn.push_tool("c1", "ls", ToolVerb::Run, ToolStatus::InProgress);
    turn.push_tool(
        "c2",
        "a much longer command line than fits",
        ToolVerb::Run,
        ToolStatus::InProgress,
    );
    assert!(turn.toggle_last());
    let expanded = turn.len();
    assert!(expanded > 2);
    assert!(turn.update_tool(
        "c2",
        Some("a different, even longer command line than before"),
        Some(ToolStatus::Completed),
        None
    ));
    assert!(
        turn.rows().iter().any(|r| r.text.starts_with("✓ run")),
        "the head row shows the new status"
    );
    assert!(turn.toggle_last());
    assert_eq!(turn.len(), 2, "collapsed again");
}

#[test]
fn a_check_is_one_row_collapsed_and_its_details_expand() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "gating\n");
    turn.push_check(
        "cargo test",
        None,
        true,
        Some(0),
        Some(1234),
        "348 passed\n0 failed",
    );
    assert_eq!(
        texts(&turn),
        ["gating", "✓ gate   cargo test · exit 0 · 1.2s"],
        "collapsed: one row with exit code and duration"
    );
    assert_eq!(kinds(&turn)[1], RowKind::Check(true));
    assert!(turn.toggle_at_row(1));
    let rows = texts(&turn);
    assert!(rows[2].starts_with("  348 passed"), "{rows:?}");
    assert!(rows[3].starts_with("  0 failed"), "{rows:?}");
    assert_eq!(kinds(&turn)[2], RowKind::Code, "detail rows read as code");
    // A detail row belongs to the same card: toggling there collapses.
    assert!(turn.toggle_at_row(2));
    assert_eq!(turn.len(), 2);
}

#[test]
fn a_failed_check_wears_the_failure_glyph_and_kind() {
    let mut turn = Turn::new(60);
    turn.push_check(
        "lint",
        None,
        false,
        Some(101),
        None,
        "error: unused variable",
    );
    assert_eq!(texts(&turn), ["✗ gate   lint · exit 101"]);
    assert_eq!(kinds(&turn), [RowKind::Check(false)]);
    let mut bare = Turn::new(60);
    bare.push_check("review", None, true, None, None, "");
    assert_eq!(
        texts(&bare),
        ["✓ gate   review"],
        "no exit code or duration reads bare"
    );
}

#[test]
fn events_are_one_line_rows_in_arrival_order() {
    let mut turn = Turn::new(60);
    turn.push_text(Voice::Agent, "working");
    turn.push_event("▸ task started · wire the cards");
    turn.push_event("✦ think 42 · open the iteration");
    turn.push_text(Voice::Agent, "still here");
    assert_eq!(
        texts(&turn),
        [
            "working",
            "▸ task started · wire the cards",
            "✦ think 42 · open the iteration",
            "still here"
        ]
    );
    assert_eq!(kinds(&turn)[1], RowKind::Event);
    assert_eq!(kinds(&turn)[2], RowKind::Event);
    assert!(!turn.toggle_at_row(1), "event rows have no fold");
}

#[test]
fn toggle_last_prefers_the_newest_foldable_card() {
    let mut turn = Turn::new(30);
    turn.push_tool("c1", "ls", ToolVerb::Run, ToolStatus::Completed);
    turn.push_check("cargo test", None, true, Some(0), None, "everything passed");
    turn.push_event("▸ objective shipped");
    assert!(turn.toggle_last());
    let rows = texts(&turn);
    assert!(
        rows.iter().any(|r| r.starts_with("  everything")),
        "the check card expanded, not the tool: {rows:?}"
    );
}

#[test]
fn a_push_into_a_hundred_thousand_line_turn_re_wraps_only_the_new_line() {
    let mut turn = Turn::new(78);
    for i in 0..100_000 {
        turn.push_text(Voice::Agent, &format!("line {i}\n"));
    }
    let rows = turn.len();
    let first = turn.rows()[0].clone();
    let started = std::time::Instant::now();
    for i in 0..1_000 {
        match i % 3 {
            0 => turn.push_text(Voice::Agent, &format!("tail {i}\n")),
            1 => turn.push_check("cargo test", None, true, Some(0), Some(10), "ok"),
            _ => turn.push_event("▸ task started · more"),
        }
    }
    let elapsed = started.elapsed();
    assert_eq!(turn.len(), rows + 1_000, "each push lands as one row");
    assert_eq!(turn.rows()[0], first, "earlier rows are untouched");
    assert_eq!(
        turn.lines().last().map(Line::row_start),
        Some(rows + 999),
        "the new line anchors at the old tail"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "1000 pushes into a 100K-line turn must not re-wrap the transcript (took {elapsed:?})"
    );
}

const BODY: &str = "@@ -1,3 +1,3 @@\n fn main() {\n-    old();\n+    new();\n }";

#[test]
fn an_edit_is_one_collapsed_row_with_counts() {
    let mut turn = Turn::new(60);
    turn.push_edit("e1", "src/main.rs", ToolStatus::InProgress, BODY);
    assert_eq!(texts(&turn), ["● edit   src/main.rs · +1 −1"]);
    assert_eq!(kinds(&turn), [RowKind::Tool(ToolStatus::InProgress)]);
}

#[test]
fn an_expanded_edit_shows_buttons_and_styled_diff_rows() {
    let mut turn = Turn::new(60);
    turn.push_edit("e1", "src/main.rs", ToolStatus::Completed, BODY);
    assert!(turn.toggle_last());
    assert_eq!(
        texts(&turn),
        [
            "✓ edit   src/main.rs · +1 −1",
            "  [Open]  [Undo]",
            "  @@ -1,3 +1,3 @@",
            "   fn main() {",
            "  -    old();",
            "  +    new();",
            "   }",
        ]
    );
    assert_eq!(
        kinds(&turn),
        [
            RowKind::Tool(ToolStatus::Completed),
            RowKind::EditButtons,
            RowKind::Fence,
            RowKind::Code,
            RowKind::DiffRemove,
            RowKind::DiffAdd,
            RowKind::Code,
        ]
    );
    // A click anywhere in the card folds it back to one row.
    assert!(turn.toggle_at_row(4));
    assert_eq!(turn.len(), 1);
}

#[test]
fn a_tool_update_stamps_the_duration_on_its_head_row() {
    let mut turn = Turn::new(40);
    turn.push_tool("c1", "cargo test", ToolVerb::Run, ToolStatus::InProgress);
    assert!(turn.update_tool("c1", None, Some(ToolStatus::Completed), Some(842)));
    assert_eq!(texts(&turn), ["✓ run    cargo test · 842ms"]);
}

#[test]
fn each_row_carries_its_lines_accent() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::User, "fix it\n");
    turn.push_text(Voice::Thought, "hmm\n");
    turn.push_text(Voice::Agent, "On it.\n");
    turn.push_tool("c1", "ls", ToolVerb::Run, ToolStatus::InProgress);
    turn.push_note("a note");
    assert_eq!(turn.accent_at_row(0), Accent::User);
    assert_eq!(turn.accent_at_row(1), Accent::Thought);
    assert_eq!(turn.accent_at_row(2), Accent::Agent);
    assert_eq!(turn.accent_at_row(3), Accent::Tool);
    assert_eq!(turn.accent_at_row(4), Accent::Meta);
    assert_eq!(Turn::new(10).accent_at_row(0), Accent::Meta, "empty turn");
}

#[test]
fn an_edit_update_carries_a_tool_row_into_the_card() {
    let mut turn = Turn::new(60);
    turn.push_tool("e1", "edit src/lib.rs", ToolVerb::Edit, ToolStatus::Pending);
    turn.set_edit("e1", "src/lib.rs", Some(ToolStatus::Completed), "+one");
    assert_eq!(texts(&turn), ["✓ edit   src/lib.rs · +1 −0"]);
    // A status-only tool update still lands on the edit card.
    assert!(turn.update_tool("e1", None, Some(ToolStatus::Failed), None));
    assert_eq!(texts(&turn), ["✗ edit   src/lib.rs · +1 −0"]);
}

#[test]
fn set_edit_without_a_known_call_pushes_the_card() {
    let mut turn = Turn::new(60);
    turn.set_edit("e9", "a.txt", None, "+a");
    assert_eq!(texts(&turn), ["○ edit   a.txt · +1 −0"]);
}

#[test]
fn undo_marks_the_card_and_drops_its_undo_button() {
    let mut turn = Turn::new(60);
    turn.push_edit("e1", "src/main.rs", ToolStatus::Completed, BODY);
    assert!(turn.undo_edit("e1"));
    assert_eq!(texts(&turn), ["✓ edit   src/main.rs · +1 −1 · undone"]);
    assert!(turn.toggle_last());
    assert_eq!(texts(&turn)[1], "  [Open]", "no Undo after the rewind");
    assert!(!turn.undo_edit("nope"), "an unknown id is refused");
}

#[test]
fn edit_at_row_names_the_card_under_any_of_its_rows() {
    let mut turn = Turn::new(60);
    turn.push_text(Voice::Agent, "editing\n");
    turn.push_edit("e1", "src/main.rs", ToolStatus::Completed, BODY);
    assert_eq!(turn.edit_at_row(1), Some(("e1", "src/main.rs", false)));
    assert!(turn.toggle_last());
    assert_eq!(turn.edit_at_row(2), Some(("e1", "src/main.rs", false)));
    assert_eq!(turn.edit_at_row(0), None, "prose is no edit");
    turn.undo_edit("e1");
    assert_eq!(turn.edit_at_row(1), Some(("e1", "src/main.rs", true)));
}

/// A PNG of `width × height`, all one colour.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(200, 40, 40, 255));
    pixmap.encode_png().expect("a pixmap encodes")
}

fn stream_image(width: u32, height: u32) -> StreamImage {
    StreamImage::decode(&png(width, height), "a chart", CellSize::new(10, 20))
        .expect("a png decodes")
}

#[test]
fn an_image_takes_as_many_rows_as_its_footprint() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "look:\n");
    // 40×60 px at 10×20 px cells is 4 columns by 3 rows.
    turn.push_image(stream_image(40, 60), "a chart");
    assert_eq!(turn.len(), 4, "one prose row and three image rows");
    assert_eq!(
        kinds(&turn)[1..],
        [RowKind::Image, RowKind::Image, RowKind::Image]
    );
    assert_eq!(texts(&turn)[1], "    ", "an image row is as wide as it is");
}

#[test]
fn an_image_carries_the_agent_accent_and_is_found_by_row() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "look:\n");
    turn.push_image(stream_image(40, 60), "a chart");
    assert_eq!(turn.accent_at_row(1), Accent::Agent);
    let (image, offset, (cols, rows)) = turn.image_at_row(2).expect("row 2 is inside the image");
    assert_eq!(image.alt(), "a chart");
    assert_eq!(offset, 1, "the second row of the placement");
    assert_eq!((cols, rows), (4, 3));
    assert!(
        turn.image_at_row(0).is_none(),
        "the prose row is not an image"
    );
    assert!(turn.image_at_row(9).is_none(), "past the end");
}

#[test]
fn an_image_re_wraps_with_the_stage_width() {
    let mut turn = Turn::new(80);
    // 800×400 px is 80×20 cells naturally, capped to 20 rows.
    turn.push_image(stream_image(800, 400), "a chart");
    assert_eq!(turn.len(), 20);
    turn.rewrap(20);
    assert_eq!(turn.len(), 5, "a quarter as wide is a quarter as tall");
    assert_eq!(texts(&turn)[0].chars().count(), 20);
}

#[test]
fn a_new_cell_size_re_measures_every_image() {
    let mut turn = Turn::new(80);
    turn.push_image(stream_image(40, 60), "a chart");
    assert_eq!(turn.len(), 3);
    turn.set_cell_size(CellSize::new(20, 40));
    assert_eq!(turn.cell_size(), CellSize::new(20, 40));
    assert_eq!(turn.len(), 2, "bigger cells, fewer of them");
    let (image, _, _) = turn.image_at_row(0).expect("the image is still there");
    assert_eq!(image.cell_size(), CellSize::new(20, 40));
}

#[test]
fn an_image_closes_the_open_prose_line() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "here it is:");
    turn.push_image(stream_image(40, 20), "a chart");
    turn.push_text(Voice::Agent, "and that is it");
    assert_eq!(turn.lines().len(), 3);
    assert_eq!(texts(&turn)[0], "here it is:");
    assert_eq!(texts(&turn)[2], "and that is it");
}

#[test]
fn a_tool_calls_image_lands_after_its_card_and_never_twice() {
    let mut turn = Turn::new(40);
    turn.push_tool("tc-1", "screenshot", ToolVerb::Tool, ToolStatus::InProgress);
    // 40×60 px at 10×20 px cells is 4 columns by 3 rows.
    turn.set_tool_image("tc-1", 0, Some(stream_image(40, 60)), "a shot");
    assert_eq!(turn.len(), 4, "the card's row and three image rows");
    assert_eq!(
        kinds(&turn),
        [
            RowKind::Tool(ToolStatus::InProgress),
            RowKind::Image,
            RowKind::Image,
            RowKind::Image
        ]
    );
    let (image, _, _) = turn.image_at_row(1).expect("the image follows the card");
    assert_eq!(image.alt(), "a chart");

    // The completion update re-sends the same block: the line it
    // already pushed is found again, so the card's rows do not move.
    turn.update_tool("tc-1", None, Some(ToolStatus::Completed), Some(120));
    turn.set_tool_image("tc-1", 0, Some(stream_image(40, 60)), "a shot");
    assert_eq!(turn.len(), 4, "no second copy under the card");
    assert_eq!(turn.lines().len(), 2);
    assert_eq!(
        turn.rows()[0].kind,
        RowKind::Tool(ToolStatus::Completed),
        "the card is still row 0"
    );
}

#[test]
fn each_block_of_a_call_gets_its_own_line_and_a_new_one_replaces_in_place() {
    let mut turn = Turn::new(40);
    turn.push_tool("tc-1", "screenshot", ToolVerb::Tool, ToolStatus::InProgress);
    turn.set_tool_image("tc-1", 0, Some(stream_image(40, 60)), "first");
    turn.set_tool_image("tc-1", 1, Some(stream_image(40, 40)), "second");
    assert_eq!(turn.lines().len(), 3, "the card and one line per block");
    assert_eq!(turn.len(), 1 + 3 + 2);

    // The same ordinal carrying different pixels replaces the line.
    turn.set_tool_image("tc-1", 0, Some(stream_image(40, 20)), "first");
    assert_eq!(turn.lines().len(), 3, "replaced, not appended");
    assert_eq!(turn.len(), 1 + 1 + 2, "the shorter image takes one row");
}

#[test]
fn a_tool_calls_undecodable_block_is_one_note_and_stays_one() {
    let mut turn = Turn::new(40);
    turn.push_tool("tc-1", "screenshot", ToolVerb::Tool, ToolStatus::InProgress);
    turn.set_tool_image("tc-1", 0, None, "[image image/jpeg] could not be shown");
    assert_eq!(
        kinds(&turn),
        [RowKind::Tool(ToolStatus::InProgress), RowKind::Note]
    );
    assert_eq!(texts(&turn)[1], "[image image/jpeg] could not be shown");
    // Re-sent on the completion update: still one note.
    turn.set_tool_image("tc-1", 0, None, "[image image/jpeg] could not be shown");
    assert_eq!(turn.len(), 2);
}

#[test]
fn two_calls_sending_the_same_block_each_keep_their_own_line() {
    let mut turn = Turn::new(40);
    turn.push_tool("tc-1", "shot one", ToolVerb::Tool, ToolStatus::Completed);
    turn.set_tool_image("tc-1", 0, Some(stream_image(40, 20)), "a shot");
    turn.push_tool("tc-2", "shot two", ToolVerb::Tool, ToolStatus::Completed);
    turn.set_tool_image("tc-2", 0, Some(stream_image(40, 20)), "a shot");
    assert_eq!(
        kinds(&turn),
        [
            RowKind::Tool(ToolStatus::Completed),
            RowKind::Image,
            RowKind::Tool(ToolStatus::Completed),
            RowKind::Image
        ],
        "the key is the call, not the pixels"
    );
}

#[test]
fn a_new_row_cap_re_measures_every_image() {
    let mut turn = Turn::new(80);
    assert_eq!(turn.row_cap(), MAX_IMAGE_ROWS);
    // 100×2000 px at 10×20 px cells is 100 rows naturally, cut to the cap.
    turn.push_image(stream_image(100, 2000), "a tall chart");
    assert_eq!(turn.len(), usize::from(MAX_IMAGE_ROWS));
    turn.set_row_cap(100);
    assert_eq!(turn.row_cap(), 100);
    assert_eq!(
        turn.len(),
        100,
        "the window has the room, so the picture stands"
    );
    let (image, _, (cols, rows)) = turn.image_at_row(0).expect("the image is still there");
    assert_eq!(image.row_cap(), 100);
    assert_eq!((cols, rows), (10, 100));
    // An image arriving after the cap was set is measured against it.
    turn.push_image(stream_image(100, 2000), "another");
    assert_eq!(turn.len(), 200);
    // The same cap again is not a re-wrap.
    let before = turn.clone();
    turn.set_row_cap(100);
    assert_eq!(turn, before);
}

#[test]
fn card_at_row_answers_the_tool_or_edit_head_and_nothing_else() {
    let mut turn = Turn::new(20);
    turn.push_text(Voice::Agent, "prose before\n");
    turn.push_tool(
        "tc-1",
        "Run the gates",
        ToolVerb::Run,
        ToolStatus::InProgress,
    );
    turn.push_text(Voice::Agent, "prose after");
    let head = turn.lines()[1].row_start();
    let (line, span) = turn.card_at_row(head).expect("the tool line cards");
    assert!(matches!(line.kind(), LineKind::Tool { id, .. } if id == "tc-1"));
    assert_eq!(
        head + span,
        turn.lines()[2].row_start(),
        "the span runs to the next line's first row"
    );
    assert!(turn.card_at_row(0).is_none(), "a prose row never cards");
    assert!(
        turn.card_at_row(head + span).is_none(),
        "the row after the card is prose again"
    );
    assert!(
        Turn::new(20).card_at_row(0).is_none(),
        "an empty turn cards nothing"
    );
}

/// A check line cards too: the window opens it into the gate's own
/// rows, so its head answers like a tool's.
#[test]
fn card_at_row_cards_a_check_line() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "gating\n");
    turn.push_check(
        "cargo test",
        Some("cargo nextest run"),
        false,
        Some(101),
        Some(1234),
        "error: unused variable\n  --> src/main.rs:3:9",
    );
    let head = turn.lines()[1].row_start();
    let (line, span) = turn.card_at_row(head).expect("the check line cards");
    assert!(matches!(line.kind(), LineKind::Check { name, .. } if name == "cargo test"));
    assert_eq!(span, 1, "collapsed: one row");
    assert!(turn.toggle_at_row(head));
    let (_, span) = turn.card_at_row(head).expect("still cards open");
    assert_eq!(span, 3, "expanded: the head and the two detail rows");
}

/// The gate's command rides the line's record as data — the window's
/// detail names it; the grid's rows never change for it.
#[test]
fn push_check_keeps_the_gates_command_on_the_record() {
    let mut turn = Turn::new(40);
    turn.push_check("lint", Some("cargo clippy"), true, Some(0), None, "");
    assert_eq!(
        turn.lines()[0].record().input,
        Some(serde_json::Value::String("cargo clippy".to_owned()))
    );
    assert_eq!(texts(&turn), ["✓ gate   lint · exit 0"]);
    let mut bare = Turn::new(40);
    bare.push_check("review", None, true, None, None, "");
    assert!(bare.lines()[0].record().is_empty(), "no command, no record");
}

/// Esc's stop: the last open card folds, then the one before it, then
/// nothing — and a turn with no card open answers false.
#[test]
fn collapse_expanded_folds_the_last_open_card_first() {
    let mut turn = Turn::new(40);
    turn.push_tool("c1", "ls", ToolVerb::Run, ToolStatus::Completed);
    turn.push_check("test", None, true, Some(0), None, "ok");
    turn.push_tool("c2", "cat", ToolVerb::Read, ToolStatus::Completed);
    assert!(!turn.collapse_expanded(), "nothing open");
    let heads: Vec<usize> = turn.lines().iter().map(Line::row_start).collect();
    for head in heads.iter().rev() {
        assert!(turn.toggle_at_row(*head));
    }
    let open = |turn: &Turn| {
        turn.lines()
            .iter()
            .filter(|line| {
                matches!(
                    line.kind(),
                    LineKind::Tool { expanded: true, .. }
                        | LineKind::Check { expanded: true, .. }
                        | LineKind::Edit { expanded: true, .. }
                )
            })
            .count()
    };
    assert_eq!(open(&turn), 3);
    assert!(turn.collapse_expanded());
    assert!(
        matches!(
            turn.lines()[2].kind(),
            LineKind::Tool {
                expanded: false,
                ..
            }
        ),
        "the newest folded first"
    );
    assert_eq!(open(&turn), 2);
    assert!(turn.collapse_expanded());
    assert!(turn.collapse_expanded());
    assert!(!turn.collapse_expanded(), "all folded");
    assert_eq!(open(&turn), 0);
}

#[test]
fn prose_at_row_answers_the_paragraph_covering_a_row_and_its_rows_from_there() {
    let mut turn = Turn::new(10);
    turn.push_text(Voice::Agent, "one two three four five six\n");
    turn.push_tool(
        "tc-1",
        "Run the gates",
        ToolVerb::Run,
        ToolStatus::InProgress,
    );
    turn.push_text(Voice::Thought, "after");
    assert_eq!(texts(&turn)[..3], ["one two", "three four", "five six"]);
    let (line, span) = turn.prose_at_row(0).expect("row 0 is prose");
    assert_eq!(line.kind(), &LineKind::Prose(Voice::Agent));
    assert_eq!(span, 3, "the whole paragraph from its head");
    let (_, rest) = turn.prose_at_row(1).expect("a row inside the paragraph");
    assert_eq!(rest, 2, "the rows from there to the paragraph's end");
    let head = turn.lines()[1].row_start();
    assert!(
        turn.prose_at_row(head).is_none(),
        "a tool row is a card, not prose"
    );
    let (line, span) = turn
        .prose_at_row(head + 1)
        .expect("the thought after the card");
    assert_eq!(line.kind(), &LineKind::Prose(Voice::Thought));
    assert_eq!(span, 1);
    assert!(turn.prose_at_row(99).is_none(), "past the end");
    assert!(Turn::new(10).prose_at_row(0).is_none(), "an empty turn");
}

#[test]
fn an_open_card_that_embeds_a_terminal_takes_the_panes_rows_and_is_found_by_row() {
    let mut turn = Turn::new(40);
    turn.push_text(Voice::Agent, "running:\n");
    turn.push_tool("c1", "cargo test", ToolVerb::Run, ToolStatus::InProgress);
    assert!(turn.set_tool_record(
        "c1",
        ToolRecord {
            terminal: Some("term-1".to_owned()),
            ..ToolRecord::default()
        }
    ));
    assert_eq!(turn.len(), 2, "folded: the terminal takes no rows");
    assert_eq!(turn.pane_at_row(1), None);
    assert!(turn.toggle_last());
    // The head, the wrapped title, then the pane.
    let kinds = kinds(&turn);
    assert_eq!(kinds.len(), 3 + Turn::PANE_ROWS);
    assert!(kinds[3..].iter().all(|k| *k == RowKind::Pane), "{kinds:?}");
    assert!(
        texts(&turn)[3..].iter().all(|t| t.is_empty()),
        "pane rows carry no text"
    );
    assert_eq!(turn.pane_at_row(2), None, "the title row is not the pane");
    assert_eq!(turn.pane_at_row(3), Some(("term-1", 0)));
    assert_eq!(
        turn.pane_at_row(3 + Turn::PANE_ROWS - 1),
        Some(("term-1", Turn::PANE_ROWS - 1))
    );
    assert_eq!(turn.pane_at_row(3 + Turn::PANE_ROWS), None, "past the end");
    // A line after the card bounds the pane from below.
    turn.push_text(Voice::Agent, "after\n");
    assert_eq!(
        turn.pane_at_row(3 + Turn::PANE_ROWS - 1),
        Some(("term-1", Turn::PANE_ROWS - 1))
    );
    assert_eq!(
        turn.pane_at_row(3 + Turn::PANE_ROWS),
        None,
        "the prose row is not the pane"
    );
    assert_eq!(
        turn.accent_at_row(5),
        Accent::Tool,
        "the pane is the card's"
    );
    // Folding the card takes the pane with it.
    assert!(turn.toggle_at_row(1));
    assert_eq!(turn.len(), 3);
}

#[test]
fn a_terminal_named_on_a_card_already_open_grows_it_by_the_pane() {
    let mut turn = Turn::new(40);
    turn.push_tool("c1", "ls", ToolVerb::Run, ToolStatus::InProgress);
    assert!(turn.toggle_last());
    assert_eq!(turn.len(), 2, "head and title, no terminal yet");
    assert!(turn.set_tool_record(
        "c1",
        ToolRecord {
            terminal: Some("term-9".to_owned()),
            ..ToolRecord::default()
        }
    ));
    assert_eq!(turn.len(), 2 + Turn::PANE_ROWS);
    assert_eq!(turn.pane_at_row(2), Some(("term-9", 0)));
}

#[test]
fn a_check_card_takes_its_gates_terminal_by_name() {
    let mut turn = Turn::new(40);
    turn.push_check("check", Some("cargo check"), true, Some(0), Some(900), "ok");
    turn.push_check("test", None, false, Some(1), None, "3 failed");
    assert!(!turn.set_check_terminal("lint", "lint"), "no such gate");
    assert!(turn.set_check_terminal("check", "check"));
    assert_eq!(turn.len(), 2, "folded: no pane rows");
    assert!(turn.toggle_at_row(0));
    // The head, the details row, then the pane.
    assert_eq!(turn.len(), 2 + Turn::PANE_ROWS + 1);
    assert_eq!(turn.pane_at_row(2), Some(("check", 0)));
    assert_eq!(
        turn.pane_at_row(2 + Turn::PANE_ROWS),
        None,
        "the test card is not the pane"
    );
    // Naming the terminal on the open card re-wraps it at once.
    assert!(turn.toggle_at_row(turn.len() - 1));
    let before = turn.len();
    assert!(turn.set_check_terminal("test", "test"));
    assert_eq!(turn.len(), before + Turn::PANE_ROWS);
}

#[test]
fn a_progress_report_grows_the_open_card_by_a_strip_row_and_a_clear_takes_it_back() {
    let mut turn = Turn::new(40);
    turn.push_tool("c1", "cargo build", ToolVerb::Run, ToolStatus::InProgress);
    assert!(turn.set_tool_record(
        "c1",
        ToolRecord {
            terminal: Some("term-1".to_owned()),
            ..ToolRecord::default()
        }
    ));
    assert!(!turn.set_terminal_strip("term-9", true), "no such terminal");
    assert!(turn.set_terminal_strip("term-1", true));
    assert!(turn.has_strip("term-1"));
    assert!(!turn.has_strip("term-9"));
    assert!(!turn.set_terminal_strip("term-1", true), "already on");
    assert_eq!(turn.len(), 1, "folded: the strip takes no row yet");
    assert!(turn.toggle_last());
    // The head, the title row, the strip, then the screen's rows.
    assert_eq!(turn.len(), 2 + 1 + Turn::PANE_ROWS);
    assert_eq!(
        turn.pane_at_row(2),
        Some(("term-1", 0)),
        "the strip is offset 0"
    );
    assert_eq!(
        turn.pane_at_row(2 + Turn::PANE_ROWS),
        Some(("term-1", Turn::PANE_ROWS))
    );
    assert!(turn.set_terminal_strip("term-1", false));
    assert!(!turn.has_strip("term-1"));
    assert_eq!(turn.len(), 2 + Turn::PANE_ROWS, "re-wrapped without it");
}

/// A held kill must reach the right producer: a gate's terminal is the
/// loop driver's child, the agent's the transport's.
#[test]
fn a_terminal_is_a_gates_when_the_newest_card_on_it_is_a_check() {
    let mut turn = Turn::new(40);
    turn.push_tool("c1", "cargo build", ToolVerb::Run, ToolStatus::InProgress);
    assert!(turn.set_tool_record(
        "c1",
        ToolRecord {
            terminal: Some("term-1".to_owned()),
            ..ToolRecord::default()
        }
    ));
    turn.push_check("check", Some("cargo check"), true, Some(0), Some(900), "ok");
    assert!(turn.set_check_terminal("check", "check"));
    assert!(!turn.terminal_is_gate("term-1"));
    assert!(turn.terminal_is_gate("check"));
    assert!(!turn.terminal_is_gate("nobody"));
}

// ── The anchor and the one-line re-wrap (stage-scroll-anchor-and-rate) ──

/// The anchor names the line under a row and how far into its wrap the
/// row sits; after a re-wrap the same anchor resolves to the row that
/// content moved to, its offset clamped to the line's new wrap.
#[test]
fn an_anchor_resolves_the_same_content_to_its_row_after_a_rewrap() {
    let mut turn = Turn::new(20);
    turn.push_text(
        Voice::Agent,
        "one two three four five six seven eight nine ten\n",
    );
    turn.push_tool("t1", "cargo check", ToolVerb::Run, ToolStatus::Completed);
    turn.push_text(Voice::Agent, "after\n");
    // At twenty columns the first line wraps to three rows.
    let card_row = turn.lines()[1].row_start();
    assert_eq!(card_row, 3);
    let anchor = turn.anchor_at(1).expect("a row has a line under it");
    assert_eq!(anchor, Anchor::new(0, 1));
    assert_eq!(turn.row_of(anchor), 1);
    let card = turn.anchor_at(card_row).expect("the card's head row");
    assert_eq!(card, Anchor::new(1, 0));
    // Wider: the line fits one row, so the offset clamps to it and the
    // card's head is two rows nearer.
    turn.rewrap(60);
    assert_eq!(turn.row_of(anchor), 0);
    assert_eq!(turn.row_of(card), 1);
    assert_eq!(turn.row_of(card), turn.lines()[1].row_start());
    // Narrower: the line takes six rows, the offset holds and the card
    // moved down with it.
    turn.rewrap(10);
    assert_eq!(turn.row_of(anchor), 1);
    assert_eq!(turn.row_of(card), 6);
    assert_eq!(turn.row_of(card), turn.lines()[1].row_start());
    // A row past the end names the last line; a line the turn does not
    // have resolves to the end; an empty turn has no anchor at all.
    assert_eq!(
        turn.anchor_at(1_000).map(|a| a.line()),
        Some(2),
        "clamped to the last line"
    );
    assert_eq!(turn.row_of(Anchor::new(99, 0)), turn.len());
    assert!(Turn::new(20).anchor_at(0).is_none());
}

/// A card toggled on a ten-thousand-line turn rebuilds its own rows and
/// nothing else — every later line's start shifts by the body — and the
/// rows are exactly what a whole re-wrap makes.
#[test]
fn a_card_toggle_on_a_ten_thousand_line_turn_rebuilds_only_its_rows() {
    let mut turn = Turn::new(78);
    for i in 0..5_000 {
        turn.push_text(Voice::Agent, &format!("line {i}\n"));
    }
    turn.push_tool(
        "t1",
        "cargo nextest run --workspace --exclude magistr-gui with a title long enough to \
         wrap onto a second detail row under the head",
        ToolVerb::Run,
        ToolStatus::Completed,
    );
    for i in 5_000..10_000 {
        turn.push_text(Voice::Agent, &format!("line {i}\n"));
    }
    let head = turn.lines()[5_000].row_start();
    assert_eq!(head, 5_000);
    let layout = turn.layout();
    assert!(turn.toggle_at_row(head));
    assert_eq!(turn.layout(), layout + 1, "one layout change");
    let body = turn.len() - 10_001;
    assert!(
        (2..=4).contains(&body),
        "the title wraps to a few rows: {body}"
    );
    assert_eq!(
        turn.rebuilt_rows(),
        1 + body,
        "the card's rows and nothing else"
    );
    assert_eq!(turn.lines()[5_001].row_start(), head + 1 + body);
    assert_eq!(turn.lines()[9_999].row_start(), 9_999 + body);
    let mut full = turn.clone();
    full.rewrap(78);
    assert_eq!(
        full.rows(),
        turn.rows(),
        "row for row what a whole re-wrap makes"
    );
    assert!(
        full.lines()
            .iter()
            .zip(turn.lines())
            .all(|(a, b)| a.row_start() == b.row_start())
    );
    assert_eq!(
        full.rebuilt_rows(),
        full.len(),
        "a whole re-wrap rebuilds everything"
    );
    // Folded again: the same accounting the other way.
    assert!(turn.toggle_at_row(head));
    assert_eq!(turn.rebuilt_rows(), 1);
    assert_eq!(turn.len(), 10_001);
    assert_eq!(turn.lines()[9_999].row_start(), 9_999);
    let mut full = turn.clone();
    full.rewrap(78);
    assert_eq!(full.rows(), turn.rows());
}

/// An open card whose detail grows — a running call's title updated, a
/// terminal named on it, a strip row — re-wraps the card alone, and the
/// rows agree with a whole re-wrap every time.
#[test]
fn an_open_card_growing_re_wraps_only_the_card() {
    let same_as_whole = |turn: &Turn| {
        let mut full = turn.clone();
        full.rewrap(turn.width());
        assert_eq!(full.rows(), turn.rows(), "row for row");
        assert!(
            full.lines()
                .iter()
                .zip(turn.lines())
                .all(|(a, b)| a.row_start() == b.row_start()),
            "every line starts where a whole re-wrap puts it"
        );
    };
    let mut turn = Turn::new(40);
    for i in 0..100 {
        turn.push_text(Voice::Agent, &format!("line {i}\n"));
    }
    turn.push_tool("t1", "short", ToolVerb::Run, ToolStatus::InProgress);
    for i in 100..200 {
        turn.push_text(Voice::Agent, &format!("line {i}\n"));
    }
    assert!(turn.toggle_last());
    assert_eq!(turn.rebuilt_rows(), 2, "the head and one detail row");
    same_as_whole(&turn);
    let long = "a".repeat(100);
    assert!(turn.update_tool("t1", Some(&long), Some(ToolStatus::Completed), Some(12)));
    assert_eq!(turn.rebuilt_rows(), 1 + 100_usize.div_ceil(38));
    same_as_whole(&turn);
    let record = ToolRecord {
        terminal: Some("term-1".to_owned()),
        ..Default::default()
    };
    assert!(turn.set_tool_record("t1", record));
    assert_eq!(
        turn.rebuilt_rows(),
        1 + 100_usize.div_ceil(38) + Turn::PANE_ROWS,
        "the pane's rows on the card"
    );
    same_as_whole(&turn);
    assert!(turn.set_terminal_strip("term-1", true));
    assert_eq!(
        turn.rebuilt_rows(),
        2 + 100_usize.div_ceil(38) + Turn::PANE_ROWS,
        "and the strip row"
    );
    same_as_whole(&turn);
    assert_eq!(turn.lines()[200].row_start(), turn.len() - 1);
}
