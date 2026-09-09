use super::*;

/// A sample diff, as a unified body.
const BODY: &str = "@@ -12,4 +12,6 @@\n \
    /// What a stylesheet should paint a slot with.\n\
-pub fn ink(field: PaletteField) -> Color {\n\
+pub fn ink(field: PaletteField, palette: &Palette) -> Color {\n\
+    let floor = field.declared_floor();\n\
+    ensure_contrast(palette.color(field), palette.base(), floor)\n\
 }";

fn parsed() -> Vec<Hunk> {
    Hunk::parse(BODY)
}

#[test]
fn a_header_gives_the_hunk_its_two_starts() {
    let hunks = parsed();
    assert_eq!(hunks.len(), 1);
    assert_eq!((hunks[0].old_start, hunks[0].new_start), (12, 12));
}

#[test]
fn every_row_carries_the_number_its_own_side_gives_it() {
    let hunks = parsed();
    let numbers: Vec<(Option<u32>, Option<u32>)> = hunks[0]
        .rows
        .iter()
        .map(|row| (row.old_no, row.new_no))
        .collect();
    assert_eq!(
        numbers,
        vec![
            // context: both sides advance
            (Some(12), Some(12)),
            // the removed line: old only
            (Some(13), None),
            // the three added lines: new only
            (None, Some(13)),
            (None, Some(14)),
            (None, Some(15)),
            // context again, each side where it now stands
            (Some(14), Some(16)),
        ]
    );
}

#[test]
fn a_rows_kind_is_the_one_classifier_every_other_diff_reads() {
    let kinds: Vec<RowKind> = parsed()[0].rows.iter().map(|row| row.kind).collect();
    assert_eq!(
        kinds,
        vec![
            RowKind::Code,
            RowKind::DiffRemove,
            RowKind::DiffAdd,
            RowKind::DiffAdd,
            RowKind::DiffAdd,
            RowKind::Code,
        ]
    );
    for row in &parsed()[0].rows {
        assert_eq!(
            diff_kind(&format!("{}{}", row.marker(), row.text())),
            row.kind
        );
    }
}

#[test]
fn a_conflict_marker_is_a_conflict_whichever_side_it_arrives_on() {
    let hunks = Hunk::parse("@@ -1,2 +1,2 @@\n+<<<<<<< HEAD\n-=======\n one");
    let rows = &hunks[0].rows;
    assert_eq!(rows[0].kind, RowKind::Conflict);
    assert_eq!(rows[1].kind, RowKind::Conflict);
    // The kind folded both into `Conflict`; the numbering did not.
    assert_eq!((rows[0].old_no, rows[0].new_no), (None, Some(1)));
    assert_eq!((rows[1].old_no, rows[1].new_no), (Some(1), None));
}

#[test]
fn a_body_with_no_header_is_one_hunk_from_line_one() {
    // What `diff::body` writes for a new file.
    let hunks = Hunk::parse("+fn one() {}\n+fn two() {}");
    assert_eq!(hunks.len(), 1);
    assert_eq!((hunks[0].old_start, hunks[0].new_start), (1, 1));
    assert_eq!(
        hunks[0]
            .rows
            .iter()
            .map(|row| row.new_no)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );
}

#[test]
fn an_empty_body_is_no_hunks_at_all() {
    assert!(Hunk::parse("").is_empty());
}

#[test]
fn a_replaced_line_is_split_into_the_runs_the_word_diff_found() {
    let hunks = parsed();
    let removed = &hunks[0].rows[1];
    let added = &hunks[0].rows[2];
    // The point of the word level: what the two lines share is *not*
    // marked, so the emphasis names the change rather than the line.
    let kept: String = removed
        .spans
        .iter()
        .filter(|span| !span.changed)
        .map(|span| span.text.as_str())
        .collect();
    assert!(
        kept.contains("pub fn ink(field: PaletteField"),
        "kept {kept:?}"
    );
    assert!(kept.contains("Color {"), "kept {kept:?}");
    assert_eq!(removed.text(), "pub fn ink(field: PaletteField) -> Color {");
    // The new line names what arrived, and nothing else.
    let changed: String = added
        .spans
        .iter()
        .filter(|span| span.changed)
        .map(|span| span.text.as_str())
        .collect();
    assert!(changed.contains("palette"), "changed run was {changed:?}");
    assert!(
        !changed.contains("pub fn ink"),
        "changed run was {changed:?}"
    );
    assert_eq!(
        added.text(),
        "pub fn ink(field: PaletteField, palette: &Palette) -> Color {"
    );
}

#[test]
fn an_unpaired_addition_is_not_word_diffed_against_nothing() {
    // The second and third `+` rows have no removal opposite them.
    let hunks = parsed();
    for row in &hunks[0].rows[3..=4] {
        assert_eq!(row.spans.len(), 1);
        assert!(!row.spans[0].changed);
    }
}

#[test]
fn adjacent_runs_of_one_flag_are_one_span() {
    let spans = Hunk::runs("a b c", "a x y c", ChangeTag::Insert);
    // Not one span per character: the whole insertion is one run.
    let changed: Vec<&str> = spans
        .iter()
        .filter(|span| span.changed)
        .map(|span| span.text.as_str())
        .collect();
    assert_eq!(changed.len(), 1, "{spans:?}");
}

#[test]
fn a_context_line_is_on_both_sides_and_a_replacement_is_paired() {
    let hunks = parsed();
    let pairs = hunks[0].side_by_side();
    let shape: Vec<(bool, bool)> = pairs
        .iter()
        .map(|(old, new)| (old.is_some(), new.is_some()))
        .collect();
    assert_eq!(
        shape,
        vec![
            // context
            (true, true),
            // the replacement: old line beside the line replacing it
            (true, true),
            // the two additions with nothing opposite them
            (false, true),
            (false, true),
            // context
            (true, true),
        ]
    );
    let (old, new) = pairs[1];
    assert_eq!(old.expect("old side").kind, RowKind::DiffRemove);
    assert_eq!(new.expect("new side").kind, RowKind::DiffAdd);
    assert_eq!(pairs[0].0.expect("old"), pairs[0].1.expect("new"));
}

#[test]
fn a_removal_with_no_addition_after_it_spills_on_the_old_side() {
    let hunks = Hunk::parse("@@ -1,2 +1,1 @@\n-gone\n kept");
    let pairs = hunks[0].side_by_side();
    assert_eq!(
        pairs
            .iter()
            .map(|(old, new)| (old.is_some(), new.is_some()))
            .collect::<Vec<_>>(),
        vec![(true, false), (true, true)]
    );
}

#[test]
fn the_gutter_is_as_wide_as_the_widest_number_and_blanks_the_missing_side() {
    let hunks = parsed();
    assert_eq!(Hunk::gutter_width(&hunks), 2);
    let removed = &hunks[0].rows[1];
    assert_eq!(removed.gutter(2), "13   ");
    assert_eq!(removed.side_gutter(2, true), "13");
    assert_eq!(removed.side_gutter(2, false), "  ");
    assert_eq!(hunks[0].rows[0].gutter(2), "12 12");
}

#[test]
fn the_gutter_is_one_column_wide_for_a_diff_with_no_rows() {
    assert_eq!(Hunk::gutter_width(&[]), 1);
}

#[test]
fn a_hunk_header_that_is_not_one_is_a_body_line() {
    // A context line that happens to start with `@` is not a header.
    let hunks = Hunk::parse("@@ -1,1 +1,1 @@\n @decorator");
    assert_eq!(hunks.len(), 1);
    assert_eq!(hunks[0].rows.len(), 1);
    assert_eq!(hunks[0].rows[0].text(), "@decorator");
}
