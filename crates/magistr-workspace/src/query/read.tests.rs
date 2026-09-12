use super::*;
use crate::contracts::{ErrorKind, Freshness};
use crate::index::cache::IndexedFile;
use crate::query::{ReadRange, ReadSurround};
use crate::worker::WorkerSymbol;

fn snapshot(content: &str) -> SourceObservation {
    let content = FileContent::Text(content.into());
    SourceObservation {
        path: "src/auth.ts".into(),
        content_hash: content.content_hash(),
        content,
        freshness: Freshness::Warm,
    }
}

fn symbol(name: &str, start: u32, end: u32) -> WorkerSymbol {
    WorkerSymbol {
        name: name.into(),
        kind: "function".into(),
        start_line: start,
        start_col: 1,
        end_line: end,
        detail: None,
        doc: None,
    }
}

fn outline(source: &SourceObservation, symbols: Vec<WorkerSymbol>) -> IndexQuery<IndexedFile> {
    IndexQuery {
        rows: vec![IndexedFile {
            path: source.path.clone(),
            language: Some("typescript".into()),
            size: source.content.as_bytes().len() as u64,
            mtime_ms: 0,
            mode: 0,
            content_hash: source.content_hash.clone(),
            junk: false,
            content: None,
            symbols,
            imports: vec![],
            test_ranges: vec![],
        }],
        freshness: Freshness::Warm,
        generation: 1,
        truncated: false,
        notes: vec![],
    }
}

fn unit(response: &WorkspaceResponse<ReadRow>) -> &ReadUnit {
    let WorkspaceResponse::Success { results, .. } = response else {
        panic!("expected successful read: {response:?}");
    };
    let ReadRow::Unit(unit) = &results.rows()[0] else {
        panic!("read must begin with unit header");
    };
    unit
}

fn failure_kind(response: WorkspaceResponse<ReadRow>) -> ErrorKind {
    let WorkspaceResponse::Failure { error, .. } = response else {
        panic!("expected failure: {response:?}");
    };
    error.kind
}

#[test]
fn whole_range_empty_and_hash_describe_original_content() {
    let input = ReadInput::new("src/auth.ts");
    let content = "alpha\r\nbeta\n";
    let source = snapshot(content);
    let hash = source.content_hash.clone();
    let response = render(source, &input, None);
    assert_eq!(unit(&response).range, ReadRange { start: 1, end: 2 });
    assert_eq!(unit(&response).content_hash, hash);
    let WorkspaceResponse::Success { results, .. } = response else {
        unreachable!()
    };
    let ReadRow::Lines(segment) = &results.rows()[1] else {
        unreachable!()
    };
    assert_eq!(segment.text, "alpha\r\nbeta");
    let mut input = input;
    input.lines = Some(ReadRange { start: 8, end: 2 });
    let ranged = render(snapshot(content), &input, None);
    assert_eq!(unit(&ranged).range, ReadRange { start: 2, end: 2 });
    let empty = render(snapshot(""), &input, None);
    assert_eq!(unit(&empty).range, ReadRange { start: 0, end: 0 });
    let WorkspaceResponse::Success { results, notes, .. } = empty else {
        unreachable!()
    };
    assert_eq!(results.rows().len(), 1);
    assert!(
        notes
            .expect("empty note")
            .iter()
            .any(|note| note.contains("empty"))
    );
}

#[test]
fn budget_cut_retains_whole_lines_and_resumable_cursor() {
    let content = (1..=400)
        .map(|i| format!("line {i} content"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut input = ReadInput::new("src/auth.ts");
    input.budget = 300;
    let response = render(snapshot(&content), &input, None);
    let WorkspaceResponse::Success {
        results, budget, ..
    } = response
    else {
        panic!("budgeted read");
    };
    assert!(budget.used <= 300);
    let cut = results.truncation().expect("cut manifest");
    let ReadRow::Lines(last) = results.rows().last().expect("segment") else {
        panic!("line segment");
    };
    assert_eq!(cut.cursor(), format!("line:{}", last.end + 1));
    assert!(cut.dropped() > 0);
    input.lines = Some(ReadRange {
        start: last.end + 1,
        end: 400,
    });
    input.budget = 4000;
    let resumed = render(snapshot(&content), &input, None);
    assert_eq!(unit(&resumed).range.start, last.end + 1);
    input.budget = 1;
    assert_eq!(
        failure_kind(render(snapshot(&content), &input, None)),
        ErrorKind::BudgetTooSmall
    );
}

const SOURCE: &str = "import a from './a';\nimport b from './b';\n\nclass Owner {\n  member() {\n    return 1;\n  }\n}\nfunction member() {}";

#[test]
fn symbol_addresses_enclosing_and_imports_match_reference() {
    let symbols = vec![
        symbol("Owner", 4, 8),
        symbol("member", 5, 7),
        symbol("member", 9, 9),
    ];
    let mut input = ReadInput::new("src/auth.ts");
    input.symbol = Some("Owner.member".into());
    input.surround = ReadSurround::None;
    let source = snapshot(SOURCE);
    let index = outline(&source, symbols.clone());
    let response = render(source, &input, Some(index));
    assert_eq!(unit(&response).range, ReadRange { start: 5, end: 7 });
    input.surround = ReadSurround::Enclosing;
    let source = snapshot(SOURCE);
    let index = outline(&source, symbols.clone());
    let response = render(source, &input, Some(index));
    assert_eq!(unit(&response).range, ReadRange { start: 4, end: 8 });
    assert_eq!(
        unit(&response).enclosing.as_ref().expect("parent").name,
        "Owner"
    );
    input.surround = ReadSurround::Imports;
    let source = snapshot(SOURCE);
    let index = outline(&source, symbols);
    let response = render(source, &input, Some(index));
    assert_eq!(unit(&response).range, ReadRange { start: 5, end: 7 });
    let WorkspaceResponse::Success { results, notes, .. } = response else {
        unreachable!()
    };
    let ReadRow::Imports(imports) = &results.rows()[1] else {
        panic!("import span");
    };
    assert_eq!((imports.start, imports.end), (1, 2));
    assert!(
        notes
            .expect("heuristic note")
            .iter()
            .any(|note| note.contains("lexical scan"))
    );
}

#[test]
fn rust_qualified_member_anchor_is_fresh_and_trait_ambiguity_is_explicit() {
    let source = snapshot(
        "impl Config {\nfn load() {}\nfn hello() {}\n}\nimpl Greeter for Config {\nfn hello() {}\n}",
    );
    let mut trait_method = symbol("Config::hello", 6, 6);
    trait_method.detail = Some("Greeter".into());
    let symbols = vec![
        symbol("Config::load", 2, 2),
        symbol("Config::hello", 3, 3),
        trait_method,
    ];
    let mut input = ReadInput::new("src/auth.ts");
    input.symbol = Some("load".into());
    input.surround = ReadSurround::None;
    let index = outline(&source, symbols.clone());
    let response = render(source, &input, Some(index));
    assert_eq!(unit(&response).range.start, 2);
    assert!(matches!(
        response,
        WorkspaceResponse::Success {
            freshness: Freshness::Warm,
            ..
        }
    ));
    input.symbol = Some("Config::hello".into());
    let source = snapshot("unused");
    let index = outline(&source, symbols);
    let ErrorKind::AmbiguousSymbol { candidates } =
        failure_kind(render(source, &input, Some(index)))
    else {
        panic!("ambiguity");
    };
    assert_eq!(
        candidates,
        [
            "Config::hello (function) line 3",
            "<Config as Greeter>::hello (function) line 6"
        ]
    );
}

#[test]
fn unsafe_or_unavailable_inputs_return_typed_refusals() {
    let mut input = ReadInput::new("src/auth.ts");
    input.symbol = Some("member".into());
    assert_eq!(
        failure_kind(render(snapshot(SOURCE), &input, None)),
        ErrorKind::IndexUnavailable
    );
    let source = snapshot(SOURCE);
    let mut stale = outline(&source, vec![symbol("member", 5, 7)]);
    stale.rows[0].content_hash = "old".into();
    assert_eq!(
        failure_kind(render(source, &input, Some(stale))),
        ErrorKind::IndexUnavailable
    );
    let source = snapshot(SOURCE);
    let index = outline(&source, vec![]);
    assert_eq!(
        failure_kind(render(source, &input, Some(index))),
        ErrorKind::SymbolNotFound
    );
    let source = snapshot(SOURCE);
    let mut opening = outline(&source, vec![]);
    opening.generation = 0;
    opening.rows.clear();
    assert_eq!(
        failure_kind(render(source, &input, Some(opening))),
        ErrorKind::IndexUnavailable
    );
    input.lines = Some(ReadRange { start: 1, end: 2 });
    assert_eq!(
        failure_kind(render(snapshot(SOURCE), &input, None)),
        ErrorKind::PatternInvalid
    );
    input.symbol = None;
    input.lines = Some(ReadRange { start: 0, end: 0 });
    assert_eq!(
        failure_kind(render(snapshot(SOURCE), &input, None)),
        ErrorKind::PatternInvalid
    );
    input.lines = None;
    let mut source = snapshot("");
    source.content = FileContent::Binary(vec![0, 255]);
    assert_eq!(
        failure_kind(render(source, &input, None)),
        ErrorKind::PatternInvalid
    );
}

#[tokio::test]
async fn live_reads_work_without_spawning_worker_and_surface_path_refusals() {
    use crate::{
        config::ResolveOptions,
        worker::{WorkerOptions, WorkerSupervisor},
        workspace_policy::WorkspacePolicy,
    };
    let directory = tempfile::tempdir().expect("workspace");
    std::fs::write(directory.path().join("plain.txt"), "first\nsecond\n").expect("source");
    std::fs::write(directory.path().join("binary"), [0, 255]).expect("binary");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(directory.path())).expect("policy");
    let worker = WorkerSupervisor::new(
        std::env::current_exe().expect("test executable"),
        WorkerOptions::default(),
    )
    .expect("supervisor");
    let query = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let input = ReadInput::new("plain.txt");
    let first = query.read(&input).await;
    assert_eq!(unit(&first).range, ReadRange { start: 1, end: 2 });
    std::fs::write(directory.path().join("plain.txt"), "changed").expect("changed source");
    let second = query.read(&input).await;
    assert_ne!(unit(&first).content_hash, unit(&second).content_hash);
    assert_eq!(
        failure_kind(query.read(&ReadInput::new("missing")).await),
        ErrorKind::FileNotFound
    );
    assert_eq!(
        failure_kind(query.read(&ReadInput::new("binary")).await),
        ErrorKind::PatternInvalid
    );
    assert_eq!(
        failure_kind(query.read(&ReadInput::new("../outside")).await),
        ErrorKind::PathOutsideWorkspace
    );
    let mut symbolic = input;
    symbolic.symbol = Some("changed".into());
    assert_eq!(
        failure_kind(query.read(&symbolic).await),
        ErrorKind::IndexUnavailable
    );
}

#[test]
fn freshness_never_exceeds_snapshot_or_outline_and_bad_anchors_are_labeled() {
    let mut input = ReadInput::new("src/auth.ts");
    input.symbol = Some("member".into());
    input.surround = ReadSurround::None;
    let source = snapshot(SOURCE);
    let mut index = outline(&source, vec![symbol("member", 5, 7)]);
    index.freshness = Freshness::Warming;
    assert!(matches!(
        render(source, &input, Some(index)),
        WorkspaceResponse::Success {
            freshness: Freshness::Warming,
            ..
        }
    ));
    let mut source = snapshot(SOURCE);
    source.freshness = Freshness::Cold;
    let index = outline(&source, vec![symbol("member", 5, 7)]);
    assert!(matches!(
        render(source, &input, Some(index)),
        WorkspaceResponse::Success {
            freshness: Freshness::Cold,
            ..
        }
    ));
    let source = snapshot(SOURCE);
    let index = outline(&source, vec![symbol("member", 2, 2)]);
    let WorkspaceResponse::Success {
        freshness, notes, ..
    } = render(source, &input, Some(index))
    else {
        panic!("cold anchor response");
    };
    assert_eq!(freshness, Freshness::Cold);
    assert!(
        notes
            .expect("anchor note")
            .iter()
            .any(|note| note.contains("did not match"))
    );
}
