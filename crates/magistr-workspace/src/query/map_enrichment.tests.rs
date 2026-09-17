use super::*;

#[test]
fn outlines_cap_at_twenty_preserve_docs_and_full_lifts_the_cap() {
    let symbols = (0..25)
        .map(|index| WorkerSymbol {
            name: format!("f{index}"),
            kind: "function".into(),
            start_line: index + 1,
            start_col: 1,
            end_line: index + 1,
            detail: None,
            doc: (index == 0).then(|| "First function.".into()),
        })
        .collect();
    let enrichment = MapEnrichment {
        freshness: Freshness::Warm,
        notes: vec![],
        imports: None,
        symbols: BTreeMap::from([("a.rs".into(), symbols)]),
    };
    let mut rows = vec![MapRow::File {
        path: "a.rs".into(),
        junk: false,
        language: Some("rust".into()),
        symbols: vec![],
        dropped_symbols: None,
    }];
    enrichment.attach(&mut rows, MapDetail::Skeleton);
    assert!(matches!(&rows[0], MapRow::File { symbols, .. } if symbols.is_empty()));
    enrichment.attach(&mut rows, MapDetail::Outline);
    let MapRow::File {
        symbols,
        dropped_symbols,
        ..
    } = &rows[0]
    else {
        unreachable!()
    };
    assert_eq!(symbols.len(), 20);
    assert_eq!(*dropped_symbols, Some(5));
    assert_eq!(symbols[0].doc.as_deref(), Some("First function."));
    assert_eq!(symbols[1].doc, None);
    enrichment.attach(&mut rows, MapDetail::Full);
    assert!(
        matches!(&rows[0], MapRow::File { symbols, dropped_symbols: None, .. } if symbols.len() == 25)
    );
}

#[test]
fn import_summary_ceiling_is_the_whole_inventory_or_nothing() {
    let at_ceiling: Vec<String> = (0..MapEnrichment::FILE_LIMIT)
        .map(|n| format!("src/f{n}.rs"))
        .collect();
    let inventory = MapEnrichment::graph_inventory(&at_ceiling).expect("complete inventory");
    assert_eq!(inventory.len(), at_ceiling.len());
    let mut over = at_ceiling.clone();
    over.push("src/one_more.rs".into());
    let note = MapEnrichment::graph_inventory(&over).expect_err("ceiling note");
    assert!(
        note.contains("513 files")
            && note.contains("512-file validation ceiling")
            && note.contains("no partial counts"),
        "{note}"
    );
    // Over the ceiling the graph is absent, never a subset: no fan-in/out
    // and no use counts can be derived from what was not validated.
    let enrichment = MapEnrichment {
        freshness: Freshness::Warm,
        notes: vec![note],
        imports: None,
        symbols: BTreeMap::new(),
    };
    assert_eq!(enrichment.graph(), None);
}

#[test]
fn import_summary_counts_raw_imports_orders_ties_and_omits_empty_graph() {
    let mut enrichment = MapEnrichment {
        freshness: Freshness::Warm,
        notes: vec![],
        imports: Some(BTreeMap::new()),
        symbols: BTreeMap::new(),
    };
    assert_eq!(enrichment.graph(), None);
    enrichment.imports = Some(BTreeMap::from([
        (
            "z.rs".into(),
            vec!["alpha::A".into(), "alpha::A".into(), "beta::B".into()],
        ),
        ("a.rs".into(), vec!["beta::B".into()]),
    ]));
    let graph = enrichment.graph().expect("graph");
    assert_eq!(
        graph.fan_in,
        [
            MapImportModule {
                module: "alpha::A".into(),
                count: 2
            },
            MapImportModule {
                module: "beta::B".into(),
                count: 2
            }
        ]
    );
    assert_eq!(graph.fan_out[0].module, "z.rs");
    assert_eq!(graph.fan_out[0].count, 3);
}
