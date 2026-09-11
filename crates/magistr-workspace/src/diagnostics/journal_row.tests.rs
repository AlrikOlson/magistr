use super::*;

fn row(seq: i64, tool: &str, session: &str) -> JournalRow {
    JournalRow {
        kind: "op",
        seq,
        at: 1_755_000_000_000,
        tool: tool.into(),
        session_id: session.into(),
        undoable: true,
        undone_by: None,
        undo_of: None,
        path: Some("src/a.ts".into()),
        change: Some("write"),
        added: 1,
        removed: 1,
        parse_ok: Some(true),
        binary: None,
        evicted: None,
        diff: Some("--- a\n+++ b\n-const a = 1\n+const a = 2".into()),
        diff_truncated: None,
    }
}

#[test]
fn the_text_rendering_names_the_op_the_file_and_the_counts() {
    let text = JournalRow::render_text(&[
        JournalRow {
            diff: None,
            ..row(2, "files", "bob")
        },
        JournalRow {
            change: Some("create"),
            removed: 0,
            diff: None,
            ..row(1, "edit", "")
        },
    ]);
    assert_eq!(
        text,
        "#2  2025-08-12T12:00:00.000Z  files  session=bob  undoable\n\
         \x20   M src/a.ts  +1 -1\n\
         #1  2025-08-12T12:00:00.000Z  edit  undoable\n\
         \x20   A src/a.ts  +1 -0"
    );
}

#[test]
fn diffs_render_indented_and_a_cut_diff_says_so() {
    let text = JournalRow::render_text(&[JournalRow {
        undoable: false,
        undone_by: Some(3),
        undo_of: Some(vec![1]),
        diff_truncated: Some(true),
        ..row(2, "undo", "alice")
    }]);
    assert!(text.starts_with(
        "#2  2025-08-12T12:00:00.000Z  undo  session=alice  undone by #3  (undo of 1)\n"
    ));
    assert!(text.contains("\n      +const a = 2\n      … diff cut at the per-row cap"));
}

#[test]
fn a_file_less_op_an_evicted_blob_and_an_empty_log_have_their_own_lines() {
    let thin = JournalRow {
        path: None,
        change: None,
        ..row(1, "script", "")
    };
    assert_eq!(
        JournalRow::render_text(&[thin]),
        "#1  2025-08-12T12:00:00.000Z  script  undoable\n    (no file changed)"
    );
    let evicted = JournalRow {
        evicted: Some(true),
        change: Some("delete"),
        added: 0,
        removed: 0,
        diff: None,
        ..row(1, "files", "")
    };
    assert!(
        JournalRow::render_text(&[evicted])
            .ends_with("    D src/a.ts  +0 -0  [blob evicted — history only]")
    );
    assert_eq!(JournalRow::render_text(&[]), "(the journal is empty)");
}
