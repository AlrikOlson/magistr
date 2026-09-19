use super::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
struct Row {
    path: String,
    detail: String,
}

fn row(path: &str, detail: &str) -> Row {
    Row {
        path: path.into(),
        detail: detail.into(),
    }
}

fn omit(row: &mut Row) -> bool {
    if row.detail.is_empty() {
        return false;
    }
    row.detail.clear();
    true
}

fn run(rows: Vec<Row>, requested: u64) -> WorkspaceResponse<Row> {
    receipt(
        rows,
        requested,
        Freshness::Warm,
        vec!["kept".into()],
        omit,
        |row| format!("path:{}", row.path),
    )
}

#[test]
fn a_receipt_that_fits_is_complete_and_unannotated() {
    let rows = vec![row("a", "diff a"), row("b", "diff b")];
    let WorkspaceResponse::Success {
        results,
        notes,
        budget,
        ..
    } = run(rows.clone(), 4000)
    else {
        panic!("success");
    };
    assert_eq!(results.rows(), rows);
    assert!(results.truncation().is_none());
    assert_eq!(notes, Some(vec!["kept".to_owned()]));
    assert!(budget.used <= budget.requested);
}

#[test]
fn overflowing_rows_are_compacted_never_cut_and_the_manifest_names_the_first() {
    let long = "x".repeat(400);
    let rows = vec![row("a", "ok"), row("b", &long), row("c", &long)];
    let fits_first = crate::query::budget::tokens(&rows[0]).unwrap();
    let WorkspaceResponse::Success {
        results,
        notes,
        budget,
        ..
    } = run(rows, fits_first)
    else {
        panic!("a settled mutation never refuses on budget");
    };
    let kept = results.rows();
    assert_eq!(kept.len(), 3);
    assert_eq!(kept[0].detail, "ok");
    assert!(kept[1].detail.is_empty() && kept[2].detail.is_empty());
    let manifest = results.truncation().expect("declared");
    assert_eq!(manifest.dropped(), 2);
    assert_eq!(manifest.cursor(), "path:b");
    let notes = notes.unwrap();
    assert_eq!(notes[0], "kept");
    assert!(notes[1].contains("2 diff(s) omitted") && notes[1].contains("do not re-issue"));
    assert!(
        budget.used > budget.requested,
        "the accounting stays truthful when outcomes outweigh the budget"
    );
}

#[test]
fn rows_with_nothing_to_omit_are_kept_without_a_manifest() {
    let WorkspaceResponse::Success { results, notes, .. } = run(vec![row("a", "")], 1) else {
        panic!("success");
    };
    assert_eq!(results.rows().len(), 1);
    assert!(results.truncation().is_none());
    assert_eq!(notes, Some(vec!["kept".to_owned()]));
}
