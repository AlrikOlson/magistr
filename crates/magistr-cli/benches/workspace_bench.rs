//! The native workspace perf harness (chunk arch-14c10b): the retained
//! constitution gates measured natively — tier-0 usefulness within 5 s of
//! first start, 500k LOC to tier 1 in under 60 s, references p95 under
//! 50 ms, worker round-trip p95 under 1 ms — over the retained synthetic
//! corpora, against the shipping worker binary, judged as the reference
//! benches judged them (tier 1 normalized per 500k LOC, percentiles by
//! index) and written in the `workspace-bench/1` JSON shape so the recorded
//! runs read alike beside the reference's.
//!
//! The corpus is generated in a temp dir and removed at exit; nothing here
//! measures this checkout. A failing gate is reported, not hidden.
//!
//! Usage: cargo bench -p magistr-cli --bench workspace_bench --
//!          --profile index|refs|worker [--loc N] [--json <file>] [--report-only]
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use magistr_workspace::{
    contracts::WorkspaceResponse,
    query::{MapDetail, MapInput, MapRow, ReadInput, SearchInput, SearchMode},
    service::{IndexBuildState, WorkspaceOpenOptions, WorkspaceService},
    worker::{WorkerOptions, WorkerRequest, WorkerResponse, WorkerSupervisor},
};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

const BENCH_SCHEMA: &str = "workspace-bench/1";
const TIER0_GATE_S: f64 = 5.0;
const TIER1_GATE_S: f64 = 60.0;
const TIER1_GATE_LOC: f64 = 500_000.0;
const REFS_P95_GATE_MS: f64 = 50.0;
const IPC_P95_GATE_MS: f64 = 1.0;
const LINES_PER_FILE: usize = 250;
const ITERATIONS_PER_QUERY: usize = 40;
const WARMUP_PINGS: usize = 200;
const PINGS: usize = 2000;
const MISS_NAME: &str = "bench_refs_no_such_symbol";
/// Wide enough that the hot query's whole edge set comes back, as the
/// reference's raw `index/references` frame did.
const REFS_BUDGET: u64 = 1_000_000;

/// One marker file per bundled grammar, each declaring bench_marker_<lang>.
const LANGUAGE_SAMPLES: [(&str, &str, &str); 13] = [
    (
        "bash",
        "samples/marker.sh",
        "bench_marker_bash() {\n  echo hi\n}\n",
    ),
    (
        "c",
        "samples/marker.c",
        "int bench_marker_c(void) { return 1; }\n",
    ),
    (
        "cpp",
        "samples/marker.cpp",
        "int bench_marker_cpp() { return 1; }\n",
    ),
    (
        "csharp",
        "samples/marker.cs",
        "public class BenchMarkerCsharp { public int Run() { return 1; } }\n",
    ),
    (
        "go",
        "samples/marker.go",
        "package main\n\nfunc bench_marker_go() int { return 1 }\n",
    ),
    (
        "java",
        "samples/Marker.java",
        "public class BenchMarkerJava { int run() { return 1; } }\n",
    ),
    (
        "javascript",
        "samples/marker.js",
        "function bench_marker_javascript() {}\n",
    ),
    (
        "json",
        "samples/marker.json",
        "{ \"bench_marker_json\": true }\n",
    ),
    (
        "python",
        "samples/marker.py",
        "def bench_marker_python():\n    pass\n",
    ),
    ("ruby", "samples/marker.rb", "def bench_marker_ruby\nend\n"),
    (
        "rust",
        "samples/marker.rs",
        "pub fn bench_marker_rust() -> u32 { 1 }\n",
    ),
    (
        "typescript",
        "samples/marker.ts",
        "export function bench_marker_typescript(): void {}\n",
    ),
    (
        "tsx",
        "samples/marker.tsx",
        "export function bench_marker_tsx() { return null; }\n",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    Index,
    Refs,
    Worker,
}

#[derive(Debug)]
struct Args {
    profile: Profile,
    loc: usize,
    json: Option<PathBuf>,
    report_only: bool,
}

/// `--flag value` pairs and switches; cargo's own `--bench` is ignored.
fn parse_args() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| -> Result<Option<String>, String> {
        let Some(at) = argv.iter().position(|arg| arg == flag) else {
            return Ok(None);
        };
        match argv.get(at + 1) {
            Some(next) if !next.starts_with("--") => Ok(Some(next.clone())),
            _ => Err(format!("{flag} needs a value")),
        }
    };
    let profile = match value("--profile")?.as_deref() {
        Some("index") => Profile::Index,
        Some("refs") => Profile::Refs,
        Some("worker") => Profile::Worker,
        other => {
            return Err(format!(
                "--profile must be index, refs or worker, got {}",
                other.unwrap_or("nothing")
            ));
        }
    };
    let loc = match value("--loc")? {
        None => 500_000,
        Some(raw) => raw
            .parse::<usize>()
            .ok()
            .filter(|loc| *loc > 0)
            .ok_or_else(|| format!("--loc must be a positive integer, got {raw}"))?,
    };
    Ok(Args {
        profile,
        loc,
        json: value("--json")?.map(PathBuf::from),
        report_only: argv.iter().any(|arg| arg == "--report-only"),
    })
}

fn ts_module(file_index: usize, lines: usize, shared: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut line = 0;
    if shared {
        parts.push("import { shared_ref_target } from \"../src/shared\";".into());
        line = 1;
    }
    let mut function = 0;
    while line < lines {
        let doubled = if shared {
            "  const doubled = shared_ref_target(value * 2);"
        } else {
            "  const doubled = value * 2;"
        };
        parts.push(format!(
            "export function bulk_{file_index}_{function}(value: number): number {{"
        ));
        parts.push(doubled.into());
        parts.push(format!(
            "  const label = \"file {file_index} fn {function}\";"
        ));
        parts.push("  if (doubled > 10) return doubled + label.length;".into());
        parts.push("  return doubled;".into());
        parts.push("}".into());
        line += 6;
        function += 1;
    }
    format!("{}\n", parts.join("\n"))
}

fn write(root: &Path, relative: &str, content: &str) -> usize {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("corpus dir");
    std::fs::write(&path, content).expect("corpus file");
    content.matches('\n').count()
}

/// The retained bench-index corpus: one marker per bundled grammar, the
/// plain-text needle in docs and a lockfile, then bulk TypeScript to `loc`.
fn make_index_corpus(loc: usize) -> (tempfile::TempDir, usize) {
    let dir = tempfile::Builder::new()
        .prefix("magistr-bench-index-")
        .tempdir()
        .expect("corpus");
    let root = dir.path();
    let mut written = 0;
    for (_, file, content) in LANGUAGE_SAMPLES {
        written += write(root, file, content);
    }
    write(
        root,
        "README.md",
        "bench corpus fts_needle_marker documentation\n",
    );
    write(root, "config.lock", "fts_needle_marker: locked\n");
    let files = (loc.saturating_sub(written)).div_ceil(LINES_PER_FILE);
    for index in 0..files {
        written += write(
            root,
            &format!("bulk/mod_{index}.ts"),
            &ts_module(index, LINES_PER_FILE, false),
        );
    }
    (dir, written)
}

/// The retained bench-refs corpus: every bulk module imports and calls one
/// shared symbol, so the hot query exercises the caps and the linkage.
fn make_refs_corpus(loc: usize) -> (tempfile::TempDir, usize) {
    let dir = tempfile::Builder::new()
        .prefix("magistr-bench-refs-")
        .tempdir()
        .expect("corpus");
    let root = dir.path();
    let mut written = write(
        root,
        "src/shared.ts",
        "export function shared_ref_target(value: number): number {\n  return value;\n}\n",
    );
    let files = (loc.saturating_sub(written)).div_ceil(LINES_PER_FILE);
    for index in 0..files {
        written += write(
            root,
            &format!("bulk/mod_{index}.ts"),
            &ts_module(index, LINES_PER_FILE, true),
        );
    }
    (dir, written)
}

/// Source LOC over what the index covers: newlines in the generated source
/// files (the docs and the lockfile are not source), per language.
fn corpus_facts(root: &Path, kind_files: &[(&str, &str)]) -> Value {
    let mut per_language: Vec<(String, usize, usize)> = Vec::new();
    let mut total = 0;
    let mut files = 0;
    for (language, relative) in kind_files {
        let Ok(text) = std::fs::read_to_string(root.join(relative)) else {
            continue;
        };
        let lines = text.matches('\n').count();
        total += lines;
        files += 1;
        match per_language
            .iter_mut()
            .find(|(name, _, _)| name == language)
        {
            Some(entry) => {
                entry.1 += 1;
                entry.2 += lines;
            }
            None => per_language.push(((*language).to_owned(), 1, lines)),
        }
    }
    per_language.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let languages: Vec<Value> = per_language
        .into_iter()
        .map(|(language, files, loc)| json!({"language": language, "files": files, "loc": loc}))
        .collect();
    json!({
        "kind": "synthetic",
        "loc": total,
        "files": files,
        "sourceFiles": files,
        "junkFiles": 0,
        "languages": languages,
    })
}

fn source_files(root: &Path, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = extra
        .iter()
        .map(|(language, file)| ((*language).to_owned(), (*file).to_owned()))
        .collect();
    let bulk = root.join("bulk");
    let mut names: Vec<String> = std::fs::read_dir(&bulk)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    out.extend(
        names
            .into_iter()
            .map(|name| ("typescript".to_owned(), format!("bulk/{name}"))),
    );
    out
}

async fn open(root: &Path) -> WorkspaceService {
    let options =
        WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr")).with_network_override("0");
    WorkspaceService::open(options)
        .await
        .expect("open the bench workspace")
}

fn group_hits(response: &WorkspaceResponse<magistr_workspace::query::SearchGroup>) -> usize {
    match response {
        WorkspaceResponse::Success { results, .. } => {
            results.rows().iter().map(|group| group.total_hits).sum()
        }
        WorkspaceResponse::Failure { error, .. } => {
            eprintln!("search refused: {error}");
            0
        }
    }
}

/// Poll the derived index until the build settles or the deadline passes;
/// answers whether tier 1 was reached.
async fn wait_for_tier1(service: &WorkspaceService, deadline: Instant) -> bool {
    loop {
        match service.index_task().map(|task| task.state) {
            Some(IndexBuildState::Completed) => return true,
            Some(IndexBuildState::Failed) => return false,
            _ if Instant::now() >= deadline => return false,
            _ => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
}

/// Percentiles by index, as the reference computed them; empty → zeros.
fn percentiles(samples: &[f64]) -> (f64, f64, f64, f64) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let at = |q: f64| -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let index = ((q * sorted.len() as f64).floor() as usize).min(sorted.len() - 1);
        sorted[index]
    };
    (
        at(0.5),
        at(0.95),
        at(0.99),
        sorted.last().copied().unwrap_or(0.0),
    )
}

fn round(value: f64, digits: i32) -> f64 {
    let factor = 10f64.powi(digits);
    (value * factor).round() / factor
}

fn gate(name: &str, value: f64, limit: f64, unit: &str, note: Option<String>) -> Value {
    let mut gate =
        json!({"name": name, "value": value, "limit": limit, "unit": unit, "pass": value <= limit});
    if let Some(note) = note {
        gate["note"] = Value::String(note);
    }
    gate
}

fn describe_gate(gate: &Value) -> String {
    let verdict = if gate["pass"] == true { "OK" } else { "FAIL" };
    let note = gate["note"]
        .as_str()
        .map(|note| format!(" — {note}"))
        .unwrap_or_default();
    format!(
        "{verdict}: {} {}{} (limit {}{}){note}",
        gate["name"].as_str().unwrap_or(""),
        gate["value"],
        gate["unit"].as_str().unwrap_or(""),
        gate["limit"],
        gate["unit"].as_str().unwrap_or("")
    )
}

struct Run {
    measurements: serde_json::Map<String, Value>,
    gates: Vec<Value>,
    corpus: Option<Value>,
    failures: Vec<String>,
}

impl Run {
    fn new() -> Self {
        Self {
            measurements: serde_json::Map::new(),
            gates: Vec::new(),
            corpus: None,
            failures: Vec::new(),
        }
    }

    fn measure(&mut self, key: &str, value: impl Into<Value>) {
        self.measurements.insert(key.to_owned(), value.into());
    }

    fn fail(&mut self, message: impl Into<String>) {
        let message = message.into();
        eprintln!("FAIL: {message}");
        self.failures.push(message);
    }
}

async fn bench_index(args: &Args, run: &mut Run) {
    println!("generating ~{} LOC corpus…", args.loc);
    let (dir, written) = make_index_corpus(args.loc);
    println!("corpus ready: {written} LOC at {}", dir.path().display());
    let deadline_s = TIER1_GATE_S + 30.0;
    // The clock starts at first start: spawn, self-test, the build kick and
    // the first lexical answer all count.
    let start = Instant::now();
    let service = open(dir.path()).await;
    let mut probe = SearchInput::new("bulk_0_1");
    probe.mode = SearchMode::Lexical;
    probe.path = Some("bulk/mod_0.ts".into());
    let hits = group_hits(&service.search(&probe, CancellationToken::new()).await);
    let tier0_s = start.elapsed().as_secs_f64();
    if hits == 0 {
        run.fail("tier-0 probe returned no hits");
    }
    println!("tier-0 lexical answer (open+worker+build-kick+search): {tier0_s:.2}s");
    run.measure("tier0S", round(tier0_s, 3));

    let reached = wait_for_tier1(&service, start + Duration::from_secs_f64(deadline_s)).await;
    let tier1_s = start.elapsed().as_secs_f64();
    let status = service.index_status();
    let indexed = status.parsed + status.reused;
    println!(
        "tier-1: {indexed}/{} files in {tier1_s:.2}s (freshness={}, building={})",
        status.files,
        status.freshness.as_str(),
        status.building
    );
    if !reached {
        run.fail("build did not reach tier 1");
    }
    run.measure("tier1S", round(tier1_s, 3));
    run.measure("tier1Reached", reached);
    run.measure("filesIndexed", indexed);
    run.measure("filesTotal", status.files);

    // Symbols are queryable through the index: a known bulk function
    // resolves by name, and every bundled grammar's marker file carries its
    // language and outline.
    let mut symbol = ReadInput::new("bulk/mod_0.ts");
    symbol.symbol = Some("bulk_0_1".into());
    if !matches!(
        service.read(&symbol).await,
        WorkspaceResponse::Success { .. }
    ) {
        run.fail("a known bulk function does not resolve by symbol");
    }
    let outline = service
        .map(&MapInput {
            path: "samples".into(),
            depth: 1,
            detail: MapDetail::Full,
            budget: 200_000,
        })
        .await;
    let (languages, symbolized) = marker_coverage(&outline);
    run.measure("languagesIndexed", languages.len());
    run.measure("languagesWithSymbols", symbolized);
    let missing: Vec<&str> = LANGUAGE_SAMPLES
        .iter()
        .map(|(language, _, _)| *language)
        .filter(|language| !languages.iter().any(|seen| seen == language))
        .collect();
    if missing.is_empty() {
        println!(
            "languages indexed: all {} bundled ids ({symbolized} with outlines)",
            languages.len()
        );
    } else {
        run.fail(format!(
            "languages missing from the map: {}",
            missing.join(", ")
        ));
    }
    let mut needle = SearchInput::new("fts_needle_marker");
    needle.mode = SearchMode::Lexical;
    let needle_hits = group_hits(&service.search(&needle, CancellationToken::new()).await);
    if needle_hits < 2 {
        run.fail(format!(
            "plain-text needle rows missing (matched {needle_hits}, expected >= 2)"
        ));
    } else {
        println!("plain text: {needle_hits} matches for the needle (docs + lockfile)");
    }

    let samples: Vec<(&str, &str)> = LANGUAGE_SAMPLES
        .iter()
        .map(|(language, file, _)| (*language, *file))
        .collect();
    let files = source_files(dir.path(), &samples);
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(language, file)| (language.as_str(), file.as_str()))
        .collect();
    let corpus = corpus_facts(dir.path(), &refs);
    let loc = corpus["loc"].as_f64().unwrap_or(0.0);
    println!(
        "corpus: {loc} source LOC over {} source files",
        corpus["sourceFiles"]
    );
    run.corpus = Some(corpus);

    run.gates
        .push(gate("tier0", round(tier0_s, 3), TIER0_GATE_S, "s", None));
    run.gates.push(if reached {
        let normalized = if loc > 0.0 {
            tier1_s * TIER1_GATE_LOC / loc
        } else {
            tier1_s
        };
        gate(
            "tier1",
            round(normalized, 3),
            TIER1_GATE_S,
            "s",
            Some(format!(
                "normalized per {TIER1_GATE_LOC} LOC; measured {}s over {loc} LOC",
                round(tier1_s, 2)
            )),
        )
    } else {
        let mut gate = gate("tier1", round(tier1_s, 3), TIER1_GATE_S, "s", None);
        gate["pass"] = Value::Bool(false);
        gate["note"] = Value::String(format!(
            "tier 1 not reached within {}s ({indexed}/{} files)",
            round(tier1_s, 1),
            status.files
        ));
        gate
    });
    service.shutdown().await.expect("clean shutdown");
}

/// Languages the map reports for the marker files, and how many of those
/// files carry an outline (JSON has none by construction).
fn marker_coverage(outline: &WorkspaceResponse<MapRow>) -> (Vec<String>, usize) {
    let WorkspaceResponse::Success { results, .. } = outline else {
        return (Vec::new(), 0);
    };
    let mut languages = Vec::new();
    let mut symbolized = 0;
    for row in results.rows() {
        if let MapRow::File {
            language: Some(language),
            symbols,
            ..
        } = row
        {
            if !languages.contains(language) {
                languages.push(language.clone());
            }
            if !symbols.is_empty() {
                symbolized += 1;
            }
        }
    }
    (languages, symbolized)
}

async fn bench_refs(args: &Args, run: &mut Run) {
    println!("generating ~{} LOC corpus…", args.loc);
    let (dir, written) = make_refs_corpus(args.loc);
    println!("corpus ready: {written} LOC at {}", dir.path().display());
    let start = Instant::now();
    let service = open(dir.path()).await;
    let reached = wait_for_tier1(&service, start + Duration::from_secs(120)).await;
    let status = service.index_status();
    let indexed = status.parsed + status.reused;
    run.measure("tier1S", round(start.elapsed().as_secs_f64(), 3));
    run.measure("tier1Reached", reached);
    run.measure("filesIndexed", indexed);
    run.measure("filesTotal", status.files);
    if !reached {
        run.fail(format!(
            "build did not reach tier 1 within 120s ({indexed}/{} files) — measurement not taken",
            status.files
        ));
        service.shutdown().await.expect("clean shutdown");
        return;
    }
    let files = source_files(dir.path(), &[("typescript", "src/shared.ts")]);
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(language, file)| (language.as_str(), file.as_str()))
        .collect();
    let corpus = corpus_facts(dir.path(), &refs);
    let loc = corpus["loc"].as_f64().unwrap_or(0.0);
    run.corpus = Some(corpus);

    // hot: defined once, called from every bulk module (caps in play);
    // rare: one definition, references only inside its own file; miss: none.
    let queries = [
        ("hot", "shared_ref_target", 1),
        ("rare", "bulk_5_2", 1),
        ("miss", MISS_NAME, 0),
    ];
    println!(
        "corpus: {loc} source LOC; queries: {}",
        queries
            .iter()
            .map(|(role, name, _)| format!("{role}={name}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let references = |name: &str| {
        let mut input = SearchInput::new(name);
        input.mode = SearchMode::References;
        input.budget = REFS_BUDGET;
        input
    };
    // Warmup: first calls page the index into cache like a live server's would be.
    for (_, name, _) in &queries {
        service
            .search(&references(name), CancellationToken::new())
            .await;
    }
    let mut samples: Vec<f64> = Vec::new();
    let mut per_query: Vec<(Vec<f64>, usize)> = vec![(Vec::new(), 0); queries.len()];
    for iteration in 0..ITERATIONS_PER_QUERY * queries.len() {
        let which = iteration % queries.len();
        let input = references(queries[which].1);
        let started = Instant::now();
        let response = service.search(&input, CancellationToken::new()).await;
        let ms = started.elapsed().as_secs_f64() * 1e3;
        samples.push(ms);
        per_query[which].0.push(ms);
        per_query[which].1 = group_hits(&response);
    }
    let (p50, p95, _, max) = percentiles(&samples);
    println!(
        "references: {} calls — p50 {p50:.2}ms, p95 {p95:.2}ms, max {max:.2}ms (hot query: {} edges)",
        samples.len(),
        per_query[0].1
    );
    let rows: Vec<Value> = queries
        .iter()
        .zip(&per_query)
        .map(|((role, name, definitions), (samples, edges))| {
            let (p50, p95, _, _) = percentiles(samples);
            println!(
                "  {role:<4} {name}: p50 {:.3}ms p95 {:.3}ms ({definitions} defs, {edges} edges)",
                p50, p95
            );
            json!({"role": role, "name": name, "definitions": definitions, "p50Ms": round(p50, 3), "p95Ms": round(p95, 3), "edges": edges})
        })
        .collect();
    run.measure("calls", samples.len());
    run.measure("p50Ms", round(p50, 3));
    run.measure("p95Ms", round(p95, 3));
    run.measure("maxMs", round(max, 3));
    run.measure("queries", Value::Array(rows));
    if per_query[0].1 == 0 {
        run.fail("no hot query returned edges — the measurement is vacuous");
    }
    run.gates.push(gate(
        "references_p95",
        round(p95, 3),
        REFS_P95_GATE_MS,
        "ms",
        Some(format!(
            "constitution states the gate at 1M LOC; measured over {loc} LOC"
        )),
    ));
    service.shutdown().await.expect("clean shutdown");
}

/// One frame round-trip through the supervised worker; `false` when the
/// answer was not the worker's `Pong`.
async fn ping(supervisor: &WorkerSupervisor, cancel: &CancellationToken) -> bool {
    match supervisor
        .call(WorkerRequest::Ping, Duration::from_secs(10), cancel)
        .await
    {
        Ok(WorkerResponse::Pong) => true,
        Ok(other) => {
            eprintln!("unexpected worker answer: {other:?}");
            false
        }
        Err(error) => {
            eprintln!("ping failed: {error}");
            false
        }
    }
}

async fn bench_worker(run: &mut Run) {
    let supervisor = WorkerSupervisor::new(
        PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
        WorkerOptions::default(),
    )
    .expect("worker supervisor");
    let cancel = CancellationToken::new();
    let mut answered = true;
    for _ in 0..WARMUP_PINGS {
        answered &= ping(&supervisor, &cancel).await;
    }
    let mut samples = Vec::with_capacity(PINGS);
    for _ in 0..PINGS {
        let started = Instant::now();
        answered &= ping(&supervisor, &cancel).await;
        samples.push(started.elapsed().as_secs_f64() * 1e3);
    }
    if !answered {
        run.fail("a ping was not answered with Pong — the measurement is not a round-trip");
    }
    let (p50, p95, p99, max) = percentiles(&samples);
    println!(
        "worker round-trip over {PINGS} pings: p50={p50:.3}ms p95={p95:.3}ms p99={p99:.3}ms max={max:.3}ms"
    );
    run.measure("pings", PINGS);
    run.measure("p50Ms", round(p50, 4));
    run.measure("p95Ms", round(p95, 4));
    run.measure("p99Ms", round(p99, 4));
    run.measure("maxMs", round(max, 4));
    run.gates.push(gate(
        "ipc_p95",
        round(p95, 4),
        IPC_P95_GATE_MS,
        "ms",
        Some(
            "the native Ping runs the parser self-test inside the worker, so this bounds the frame round-trip from above"
                .into(),
        ),
    ));
    supervisor.shutdown().await.expect("worker shutdown");
}

fn command_output(program: &str, args: &[&str], cwd: Option<&Path>) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn machine_facts() -> Value {
    let (cpu, memory_bytes) = if cfg!(target_os = "macos") {
        (
            command_output("sysctl", &["-n", "machdep.cpu.brand_string"], None),
            command_output("sysctl", &["-n", "hw.memsize"], None)
                .and_then(|text| text.parse::<u64>().ok()),
        )
    } else {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        (
            cpuinfo
                .lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split_once(':'))
                .map(|(_, model)| model.trim().to_owned()),
            meminfo
                .lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<u64>().ok())
                .map(|kb| kb * 1024),
        )
    };
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut facts = json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu": cpu.unwrap_or_else(|| "unknown".into()),
        "cores": std::thread::available_parallelism().map_or(0, usize::from),
        "memoryGb": memory_bytes.map_or(0.0, |bytes| round(bytes as f64 / 1024f64.powi(3), 1)),
        "rust": command_output("rustc", &["--version"], None).unwrap_or_else(|| "unknown".into()),
        "coreFlavor": "native",
        "worker": "magistr --workspace-worker",
    });
    if let Some(commit) = command_output("git", &["rev-parse", "HEAD"], Some(manifest_dir)) {
        facts["magistrCommit"] = Value::String(commit);
    }
    facts
}

fn profile_name(args: &Args) -> String {
    match args.profile {
        Profile::Worker => "worker-roundtrip".into(),
        Profile::Index | Profile::Refs => {
            if args.loc.is_multiple_of(1_000_000) {
                format!("synthetic-{}m", args.loc / 1_000_000)
            } else if args.loc.is_multiple_of(1000) {
                format!("synthetic-{}k", args.loc / 1000)
            } else {
                format!("synthetic-{}", args.loc)
            }
        }
    }
}

fn recorded_at() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64);
    magistr_workspace::diagnostics::utc_timestamp(millis)
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("usage: {message}");
            std::process::exit(2);
        }
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let mut run = Run::new();
    runtime.block_on(async {
        match args.profile {
            Profile::Index => bench_index(&args, &mut run).await,
            Profile::Refs => bench_refs(&args, &mut run).await,
            Profile::Worker => bench_worker(&mut run).await,
        }
    });
    if args.report_only {
        println!("report-only: gates not applied");
        for gate in &run.gates {
            println!("  {}", describe_gate(gate));
        }
    } else {
        for gate in &run.gates {
            let line = describe_gate(gate);
            println!("{line}");
            if gate["pass"] != true {
                run.failures.push(line);
            }
        }
    }
    if let Some(path) = &args.json {
        let script = match args.profile {
            Profile::Index => "bench-index",
            Profile::Refs => "bench-refs",
            Profile::Worker => "bench-ipc",
        };
        let mut report = json!({
            "schema": BENCH_SCHEMA,
            "script": script,
            "profile": profile_name(&args),
            "recordedAt": recorded_at(),
            "machine": machine_facts(),
            "measurements": Value::Object(run.measurements.clone()),
            "gates": run.gates,
            "failures": run.failures,
        });
        if let Some(corpus) = run.corpus.take() {
            report["corpus"] = corpus;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("report dir");
        }
        std::fs::write(
            path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&report).expect("report json")
            ),
        )
        .expect("write the report");
        println!("report: {}", path.display());
    }
    if !run.failures.is_empty() && !args.report_only {
        std::process::exit(1);
    }
}
