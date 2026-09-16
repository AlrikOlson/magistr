use super::TraceCardRow;
use crate::cockpit::TraceStep;

fn step() -> TraceStep {
    TraceStep {
        number: 10,
        estimated_total: 10,
        purpose: "Verify restart-safe scheduling".into(),
        context: "The lease fence and the persisted schedule share one transaction.".into(),
        thought: "Run the restart interleavings against the database, then inspect the attempt history for gaps or duplicate deliveries before the receipt is written.".into(),
        outcome: "Finish the integration checks before recording the receipt.".into(),
        next_action: "Run the retry recovery integration tests.".into(),
        rationale: "A delivery must have one durable retry budget.".into(),
        confidence: Some(0.94),
        dependencies: vec![9],
        execution_ref: Some("task:retry-tests".into()),
        pinned: true,
        is_final: true,
        ..TraceStep::default()
    }
}

fn fields(rows: &[TraceCardRow]) -> Vec<(&'static str, bool)> {
    rows.iter()
        .filter_map(|row| match row {
            TraceCardRow::Field { label, first, .. } => Some((*label, *first)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_brief_card_shows_the_three_working_fields_cut_to_two_rows() {
    let rows = TraceCardRow::rows(&step(), 60, false, true);
    assert_eq!(rows.first(), Some(&TraceCardRow::Blank));
    assert_eq!(rows.last(), Some(&TraceCardRow::Blank));
    assert_eq!(
        rows[1],
        TraceCardRow::Title("Step 10 · Verify restart-safe scheduling".into())
    );
    assert!(matches!(&rows[2], TraceCardRow::Meta(meta)
        if meta.contains("94% confidence") && meta.contains("depends on 9") && meta.contains("Saved")));
    assert!(rows.contains(&TraceCardRow::Buttons));
    let labels: Vec<&str> = fields(&rows)
        .iter()
        .filter(|(_, first)| *first)
        .map(|(l, _)| *l)
        .collect();
    assert_eq!(labels, vec!["Reasoning", "Result", "Next"]);
    let thought_rows = fields(&rows)
        .iter()
        .filter(|(l, _)| *l == "Reasoning")
        .count();
    assert_eq!(
        thought_rows,
        TraceCardRow::BRIEF_LINES,
        "a long thought is cut"
    );
    let cut = rows.iter().find_map(|row| match row {
        TraceCardRow::Field {
            label: "Reasoning",
            text,
            first: false,
            ..
        } => Some(text.clone()),
        _ => None,
    });
    assert!(
        cut.is_some_and(|text| text.ends_with('…')),
        "the cut row says so"
    );
}

#[test]
fn a_full_card_shows_every_field_whole_and_links_the_execution_ref() {
    let rows = TraceCardRow::rows(&step(), 60, true, true);
    let labels: Vec<&str> = fields(&rows)
        .iter()
        .filter(|(_, first)| *first)
        .map(|(l, _)| *l)
        .collect();
    assert_eq!(
        labels,
        vec![
            "Context",
            "Reasoning",
            "Result",
            "Next",
            "Why",
            TraceCardRow::EXECUTION_REF
        ]
    );
    let thought_rows = fields(&rows)
        .iter()
        .filter(|(l, _)| *l == "Reasoning")
        .count();
    assert!(thought_rows > TraceCardRow::BRIEF_LINES, "nothing is cut");
    assert!(
        rows.iter()
            .any(|row| matches!(row, TraceCardRow::Field { linked: true, .. }))
    );
    let unlinked = TraceCardRow::rows(&step(), 60, true, false);
    assert!(
        unlinked
            .iter()
            .all(|row| !matches!(row, TraceCardRow::Field { linked: true, .. }))
    );
}

#[test]
fn every_row_fits_the_width_it_was_wrapped_to() {
    for width in [40_u16, 60, 76, 128] {
        for full in [false, true] {
            for row in TraceCardRow::rows(&step(), width, full, true) {
                let text = match &row {
                    TraceCardRow::Field { text, .. } => text.clone(),
                    TraceCardRow::Meta(text) => text.clone(),
                    _ => continue,
                };
                let room = width
                    .saturating_sub(TraceCardRow::INDENT + TraceCardRow::LABEL_COLS)
                    .max(12);
                assert!(
                    crate::render::width(&text)
                        <= room.max(width.saturating_sub(TraceCardRow::INDENT)),
                    "{text:?} overflows {width} columns"
                );
            }
        }
    }
}

#[test]
fn a_bare_step_has_a_number_title_no_metadata_and_no_fields() {
    let rows = TraceCardRow::rows(
        &TraceStep {
            number: 3,
            ..TraceStep::default()
        },
        60,
        true,
        false,
    );
    assert_eq!(
        rows,
        vec![
            TraceCardRow::Blank,
            TraceCardRow::Title("Step 3".into()),
            TraceCardRow::Meta("No further details recorded.".into()),
            TraceCardRow::Buttons,
            TraceCardRow::Blank,
        ]
    );
}
