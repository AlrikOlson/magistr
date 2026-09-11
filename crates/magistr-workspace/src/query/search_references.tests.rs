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

fn symbol(line: u32) -> crate::worker::WorkerSymbol {
    crate::worker::WorkerSymbol {
        name: "needle".into(),
        kind: "function".into(),
        start_line: line,
        start_col: 1,
        end_line: line,
        detail: None,
        doc: None,
    }
}

#[test]
fn occurrences_are_capped_per_file_and_in_total_never_refused() {
    let content = "fn needle() {}\n".to_owned() + &"needle();\n".repeat(80);
    let mut total = 0;
    let (hits, capped) = reference_hits(&content, &[symbol(1)], "needle", true, &mut total);
    assert!(capped, "the per-file bound engaged");
    assert_eq!(hits.len(), 1 + MAX_REFERENCE_EDGES_PER_FILE);
    assert_eq!(hits[0].kind.as_deref(), Some("definition"));
    assert_eq!(hits[0].confidence, Some(SearchConfidence::High));
    assert!(
        hits[1..]
            .iter()
            .all(|hit| hit.provenance.as_deref() == Some("tier1-imports"))
    );
    assert_eq!(total, MAX_REFERENCE_EDGES_PER_FILE);

    let mut total = MAX_REFERENCE_EDGES_TOTAL;
    let (hits, capped) = reference_hits("needle();\n", &[], "needle", false, &mut total);
    assert!(hits.is_empty(), "the aggregate bound stops before any use");
    assert!(!capped, "the aggregate bound is not this file's own");
    assert_eq!(total, MAX_REFERENCE_EDGES_TOTAL);
}

#[test]
fn bare_name_matches_respect_identifier_boundaries() {
    let mut total = 0;
    let (hits, _) = reference_hits(
        "needle();\nneedles();\nmy_needle();\nx.needle\n",
        &[],
        "needle",
        false,
        &mut total,
    );
    assert_eq!(
        hits.iter().map(|hit| hit.line).collect::<Vec<_>>(),
        vec![1, 4]
    );
    assert!(
        hits.iter()
            .all(|hit| hit.confidence == Some(SearchConfidence::Low))
    );
}

#[test]
fn the_observed_freshness_is_the_worse_of_two() {
    assert_eq!(observed(Freshness::Warm, Freshness::Warm), Freshness::Warm);
    assert_eq!(
        observed(Freshness::Warm, Freshness::Warming),
        Freshness::Warming
    );
    assert_eq!(
        observed(Freshness::Warming, Freshness::Cold),
        Freshness::Cold
    );
}
