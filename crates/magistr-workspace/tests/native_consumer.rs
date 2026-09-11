//! Conformance at the ordinary Rust consumer boundary, without addon symbols.

use magistr_workspace::{engine, indexer};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixtures {
    errors: ErrorFixtures,
    parse: Vec<ParseFixture>,
    search: SearchFixture,
    rewrite: RewriteFixture,
    extract: ExtractFixture,
}

#[derive(Deserialize)]
struct ErrorFixtures {
    unknown_language: String,
    search_pattern: String,
    structural_pattern: String,
    rewrite_pattern: String,
}

#[derive(Deserialize)]
struct ParseFixture {
    language: String,
    source: String,
    root_kind: String,
    has_error: bool,
}

#[derive(Deserialize)]
struct SearchFixture {
    pattern: String,
    source: String,
    hits: Vec<SearchHit>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct SearchHit {
    line: u64,
    start: u32,
    end: u32,
    line_text: String,
}

#[derive(Deserialize)]
struct RewriteFixture {
    language: String,
    source: String,
    pattern: String,
    replacement: String,
    output: String,
    match_lines: Vec<u64>,
}

#[derive(Deserialize)]
struct ExtractFixture {
    language: String,
    source: String,
    symbols: Vec<(String, String, u32, u32)>,
    imports: Vec<String>,
}

fn fixtures() -> Fixtures {
    serde_json::from_str(include_str!(
        "../../../subsystems/workspace/conformance/engine.json"
    ))
    .expect("pinned consumer fixtures parse")
}

#[test]
fn parse_preserves_clean_and_malformed_source_diagnostics() {
    let fixtures = fixtures();
    for fixture in fixtures.parse {
        let parsed = engine::parse(&fixture.language, &fixture.source).expect("known grammar");
        assert_eq!(parsed.language, fixture.language);
        assert_eq!(parsed.root_kind, fixture.root_kind);
        assert_eq!(parsed.has_error, fixture.has_error);
        assert_eq!(parsed.error_count > 0, fixture.has_error);
        assert!(parsed.node_count > 1);
    }
    assert!(matches!(
        engine::parse(&fixtures.errors.unknown_language, "x"),
        Err(engine::CoreError::UnknownLanguage { .. })
    ));
}

#[test]
fn search_preserves_line_numbers_offsets_and_typed_pattern_errors() {
    let fixtures = fixtures();
    let fixture = fixtures.search;
    let hits = engine::search(&fixture.pattern, &fixture.source, false).expect("valid regex");
    let actual: Vec<_> = hits
        .into_iter()
        .map(|hit| SearchHit {
            line: hit.line,
            start: hit.start,
            end: hit.end,
            line_text: hit.line_text,
        })
        .collect();
    assert_eq!(actual, fixture.hits);
    assert!(matches!(
        engine::search(&fixtures.errors.search_pattern, &fixture.source, false),
        Err(engine::CoreError::BadSearchPattern { .. })
    ));
}

#[test]
fn structural_search_and_rewrite_preserve_positions_and_metavariables() {
    let fixtures = fixtures();
    let fixture = fixtures.rewrite;
    let hits = engine::structural_search(&fixture.language, &fixture.source, &fixture.pattern)
        .expect("valid structural pattern");
    assert_eq!(
        hits.iter().map(|hit| hit.line).collect::<Vec<_>>(),
        fixture.match_lines
    );
    assert!(hits.iter().all(|hit| hit.kind == "call_expression"));
    let rewritten = engine::rewrite(
        &fixture.language,
        &fixture.source,
        &fixture.pattern,
        &fixture.replacement,
    )
    .expect("valid rewrite pattern");
    assert_eq!(rewritten.match_count as usize, hits.len());
    assert_eq!(rewritten.output, fixture.output);
    assert!(matches!(
        engine::rewrite(
            &fixture.language,
            &fixture.source,
            &fixtures.errors.rewrite_pattern,
            "x"
        ),
        Err(engine::CoreError::BadRewritePattern { .. })
    ));
    assert!(matches!(
        engine::structural_search(
            &fixture.language,
            &fixture.source,
            &fixtures.errors.structural_pattern
        ),
        Err(engine::CoreError::BadStructuralPattern { .. })
    ));
}

#[test]
fn extraction_preserves_symbol_addresses_imports_and_unknown_language_errors() {
    let fixtures = fixtures();
    let fixture = fixtures.extract;
    let extracted = indexer::extract(&fixture.language, &fixture.source).expect("known grammar");
    let symbols: Vec<_> = extracted
        .symbols
        .into_iter()
        .map(|symbol| {
            (
                symbol.name,
                symbol.kind.to_owned(),
                symbol.start_line,
                symbol.start_col,
            )
        })
        .collect();
    assert_eq!(symbols, fixture.symbols);
    assert_eq!(extracted.imports, fixture.imports);
    assert!(extracted.test_ranges.is_empty());
    assert!(matches!(
        indexer::extract(&fixtures.errors.unknown_language, &fixture.source),
        Err(engine::CoreError::UnknownLanguage { .. })
    ));
}
