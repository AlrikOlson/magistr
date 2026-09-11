//! Compare native query envelopes with the retained real addon/index in independent roots.
use magistr_workspace::{
    config::ResolveOptions,
    contracts::Freshness,
    index::watch::WatchMode,
    index::{IndexOptions, WorkspaceIndex},
    query::{MapDetail, MapInput, ReadInput, ReadRange, ReadSurround, WorkspaceQueries},
    response::ResponseAdapter,
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

const FIXTURE: &str = include_str!("../../../subsystems/workspace/conformance/queries.json");

#[tokio::test]
#[ignore = "requires retained native addon; workspace-reference gate runs this explicitly"]
async fn native_queries_match_real_reference_in_independent_roots() {
    let fixtures: Value = serde_json::from_str(FIXTURE).expect("shared fixtures");
    let directory = tempfile::tempdir().expect("native root");
    let root = directory.path().join("native");
    fs::create_dir_all(root.join(".git")).expect("root");
    seed(&root, &fixtures);
    let worker = Arc::new(
        WorkerSupervisor::new(
            PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
            WorkerOptions::default(),
        )
        .expect("worker"),
    );
    let mut index = WorkspaceIndex::start(
        root.clone(),
        directory.path().join("state"),
        Arc::clone(&worker),
        IndexOptions {
            watch_mode: WatchMode::Poll,
            poll_interval: Duration::from_millis(50),
            ..Default::default()
        },
    )
    .expect("index");
    warm(&index).await;
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(&root)).expect("policy");
    let reference = reference();
    let cases = fixtures["cases"].as_array().expect("cases");
    assert_eq!(
        cases.len(),
        reference.as_array().expect("reference cases").len()
    );
    let mut mismatches = Vec::new();
    for (case, expected) in cases
        .iter()
        .zip(reference.as_array().expect("reference cases"))
    {
        assert_eq!(case["name"], expected["name"]);
        let q = WorkspaceQueries::new(
            &policy,
            &worker,
            case["warm"].as_bool().expect("warm flag").then_some(&index),
        )
        .expect("queries");
        let mut actual = execute(&q, case).await;
        let mut expected = expected["response"].clone();
        if case["name"] == "read whole cold" {
            let sealed = serde_json::to_string(&actual).expect("sealed read");
            assert!(sealed.contains("😀"), "Unicode source must survive sealing");
            assert!(sealed.contains("[REDACTED:credential-assignment]"));
            assert!(!sealed.contains("correct-horse-staple"));
        }
        native_additions(&mut actual, case);
        normalize(&mut actual, case);
        normalize(&mut expected, case);
        if actual != expected {
            mismatches.push(format!(
                "{}\nnative: {}\nreference: {}",
                case["name"], actual, expected
            ));
        }
    }
    index.shutdown().await.expect("join index");
    worker.shutdown().await.expect("reap worker");
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n\n"));
}

fn seed(root: &std::path::Path, fixtures: &Value) {
    for file in fixtures["files"].as_array().expect("files") {
        let path = root.join(file["path"].as_str().expect("path"));
        fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        fs::write(path, file["content"].as_str().expect("source")).expect("file");
    }
}

async fn warm(index: &WorkspaceIndex) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let state = index.status();
        if state.generation > 0 && !state.building && state.freshness == Freshness::Warm {
            return;
        }
        assert!(Instant::now() < deadline, "index warmup: {state:?}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn reference() -> Value {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../subsystems/workspace/conformance/queries-reference.ts");
    let output = Command::new("bun")
        .arg(script)
        .output()
        .expect("Bun reference comparator is required");
    assert!(
        output.status.success(),
        "reference failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("reference JSON")
}

async fn execute(q: &WorkspaceQueries<'_>, case: &Value) -> Value {
    let input = &case["input"];
    match case["tool"].as_str().expect("tool") {
        "search" => ResponseAdapter::seal(
            &q.search(&serde_json::from_value(input.clone()).expect("search input"))
                .await,
        )
        .expect("envelope")
        .structured()
        .clone(),
        "read" => {
            let mut request = ReadInput::new(input["file"].as_str().expect("file"));
            request.symbol = input["symbol"].as_str().map(str::to_owned);
            request.lines = input.get("lines").map(|range| ReadRange {
                start: range["start"].as_u64().expect("start") as usize,
                end: range["end"].as_u64().expect("end") as usize,
            });
            request.surround = match input["surround"].as_str() {
                Some("none") => ReadSurround::None,
                Some("imports") => ReadSurround::Imports,
                _ => ReadSurround::Enclosing,
            };
            request.budget = input["budget"].as_u64().unwrap_or(4000);
            ResponseAdapter::seal(&q.read(&request).await)
                .expect("envelope")
                .structured()
                .clone()
        }
        "map" => {
            let request = MapInput {
                path: input["path"].as_str().unwrap_or(".").into(),
                depth: input["depth"].as_u64().unwrap_or(3) as usize,
                detail: match input["detail"].as_str() {
                    Some("skeleton") => MapDetail::Skeleton,
                    Some("full") => MapDetail::Full,
                    _ => MapDetail::Outline,
                },
                budget: input["budget"].as_u64().unwrap_or(6000),
            };
            ResponseAdapter::seal(&q.map(&request).await)
                .expect("envelope")
                .structured()
                .clone()
        }
        tool => panic!("unknown fixture tool {tool}"),
    }
}

/// Cache generation and wall-clock build timestamps describe each independent
/// build, not source identity. Normalize them and adjust only their row cost.
fn normalize(response: &mut Value, case: &Value) {
    if case["tool"] != "map" {
        return;
    }
    let Some(rows) = response["results"].as_array_mut() else {
        return;
    };
    let before: u64 = rows.iter().map(tokens).sum();
    for row in rows.iter_mut() {
        if let Some(index) = row.get_mut("index").and_then(Value::as_object_mut) {
            let generation = index
                .get("generation")
                .and_then(Value::as_u64)
                .expect("map index generation must be present and nonnegative");
            if case["warm"] == true {
                assert!(
                    generation > 0,
                    "warm map must report a completed generation"
                );
            }
            index.insert("generation".into(), json!(1));
            index.remove("buildStartedAtMs");
            index.remove("buildUpdatedAtMs");
        }
    }
    let after: u64 = rows.iter().map(tokens).sum();
    let used = response["budget"]["used"].as_u64().expect("budget");
    response["budget"]["used"] = json!(used - before + after);
}

/// Assert the documented native diagnostics, then compare every remaining note.
fn native_additions(response: &mut Value, case: &Value) {
    // Early typed refusals do not carry successful query diagnostics.
    if response["ok"] != true {
        return;
    }
    let mut additions = Vec::new();
    if case["warm"] == true
        && (case["tool"] == "map"
            || (case["tool"] == "read" && case["input"]["symbol"].is_string()))
    {
        additions.extend([
            "Index generation is cache progress, not a source revision or completion evidence.",
            "Filesystem detection: default-local; requested journal mode WAL.",
            "Applied journal mode: WAL.",
        ]);
    }
    if case["tool"] == "map" {
        additions.push(
            "plugin inventory is unavailable in the native query library; no plugins are shown",
        );
    }
    if additions.is_empty() {
        return;
    }
    let notes = response["notes"]
        .as_array_mut()
        .expect("native diagnostics");
    for addition in additions {
        let position = notes
            .iter()
            .position(|note| note == addition)
            .unwrap_or_else(|| panic!("missing native diagnostic: {addition}"));
        notes.remove(position);
    }
    if notes.is_empty() {
        response.as_object_mut().expect("response").remove("notes");
    }
}

fn tokens(row: &Value) -> u64 {
    (serde_json::to_string(row)
        .expect("row")
        .encode_utf16()
        .count() as u64)
        .div_ceil(4)
}
