use super::*;

#[test]
fn imports_use_the_retained_module_segment_heuristic() {
    for target in [
        "needle",
        "./needle",
        "./needle.ts",
        "needle.core",
        "pkg.needle",
        "needle::X",
        "crate::needle",
        "crate::needle::X",
    ] {
        assert!(import_links(target, "needle"), "{target}");
    }
    assert!(!import_links("./needless", "needle"));
    assert!(!import_links("crate::needless::X", "needle"));
}

#[test]
fn unavailable_index_notes_refuse_without_confusing_valid_watcher_off_results() {
    for note in [
        "Index is opening; cold source queries remain available.",
        "Index is updating; retry or use cold source queries.",
        "Index is stopped; use cold source queries.",
    ] {
        assert!(is_index_unavailable(&[note.into()]));
    }
    assert!(!is_index_unavailable(&["Watcher disabled; queries revalidate candidates and may miss newly added files until refresh.".into()]));
}

#[test]
fn definitions_obey_both_reference_candidate_limits() {
    let symbol = crate::worker::WorkerSymbol {
        name: "needle".into(),
        kind: "function".into(),
        start_line: 1,
        start_col: 1,
        end_line: 1,
        detail: None,
        doc: None,
    };
    let mut total = 0;
    let error = reference_hits(
        "fn needle() {}",
        &vec![symbol.clone(); 201],
        "needle",
        true,
        &mut total,
    )
    .expect_err("definitions must respect the per-file bound");
    assert_eq!(error.kind, ErrorKind::BudgetTooSmall);
    let mut total = 20_000;
    let error = reference_hits("fn needle() {}", &[symbol], "needle", true, &mut total)
        .expect_err("definitions must respect the aggregate bound");
    assert_eq!(error.kind, ErrorKind::BudgetTooSmall);
}
