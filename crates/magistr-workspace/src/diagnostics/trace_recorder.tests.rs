use super::*;
use crate::{contracts::ErrorKind, diagnostics::SpanOutcome, diagnostics::SpanStatus};

fn read_trace(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .split('\n')
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn assert_contract(records: &[serde_json::Value]) {
    const RECORD_KEYS: &[&str] = &[
        "v",
        "traceId",
        "spanId",
        "parentSpanId",
        "name",
        "startUnixMs",
        "durationMs",
        "status",
        "attributes",
    ];
    assert!(!records.is_empty());
    for record in records {
        for key in record.as_object().expect("object").keys() {
            assert!(RECORD_KEYS.contains(&key.as_str()), "unknown field {key}");
        }
        assert_eq!(record["v"], 1);
        assert!(
            record["traceId"]
                .as_str()
                .is_some_and(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
        );
        assert!(
            record["spanId"]
                .as_str()
                .is_some_and(|id| id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit()))
        );
        let name = record["name"].as_str().expect("name");
        assert!(name.starts_with("tool:") || name.starts_with("engine:"));
        assert!(record["durationMs"].as_f64().is_some_and(|ms| ms >= 0.0));
        if record["status"]["ok"] == false {
            assert!(
                record["status"]["code"].is_string() && record["status"]["recovery"].is_string()
            );
        }
        let allowed: &[&str] = if name.starts_with("tool:") {
            &["tool", "envelopeV", "notesCount", "truncated"]
        } else {
            &["method"]
        };
        for key in record["attributes"].as_object().expect("attributes").keys() {
            assert!(
                allowed.contains(&key.as_str()),
                "attribute {key} outside allowlist"
            );
        }
        if let Some(tool) = name.strip_prefix("tool:") {
            assert!(record.get("parentSpanId").is_none());
            assert_eq!(record["attributes"]["tool"], tool);
        } else if let Some(method) = name.strip_prefix("engine:") {
            assert_eq!(record["attributes"]["method"], method);
        }
    }
}

#[test]
fn one_root_line_per_request_with_taxonomy_status_and_linked_children() {
    let dir = tempfile::tempdir().expect("state");
    let path = dir.path().join("trace.jsonl");
    let recorder = TraceRecorder::new(&path);
    assert!(recorder.is_enabled());
    recorder.start_tool_span("workspace_status").end(
        &SpanStatus::Ok,
        SpanOutcome {
            envelope_v: Some(1),
            ..Default::default()
        },
    );
    let search = recorder.start_tool_span("search");
    search.engine_child("searchLexical").end(&SpanStatus::Ok);
    let (trace_id, span_id) = (search.trace_id().to_owned(), search.span_id().to_owned());
    search.end(
        &SpanStatus::Failed(ErrorKind::IndexUnavailable),
        SpanOutcome {
            envelope_v: Some(1),
            notes_count: Some(2),
            truncated: Some(false),
        },
    );
    // A span never ended emits nothing.
    drop(recorder.start_tool_span("abandoned"));
    recorder.settle();

    let records = read_trace(&path);
    assert_contract(&records);
    let names: Vec<_> = records
        .iter()
        .map(|record| record["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        names,
        [
            "tool:workspace_status",
            "engine:searchLexical",
            "tool:search"
        ]
    );
    assert_eq!(records[0]["status"], serde_json::json!({ "ok": true }));
    assert_eq!(
        records[2]["status"],
        serde_json::json!({ "ok": false, "code": "INDEX_UNAVAILABLE", "recovery": "retry" })
    );
    assert_eq!(records[2]["attributes"]["notesCount"], 2);
    assert_eq!(records[1]["traceId"], trace_id);
    assert_eq!(records[1]["parentSpanId"], span_id);
    assert!(
        !records
            .iter()
            .any(|record| record["name"] == "tool:abandoned")
    );
}

#[test]
fn an_unwritable_destination_changes_nothing_and_creates_nothing() {
    let dir = tempfile::tempdir().expect("state");
    let broken_dir = dir.path().join("no-such-dir").join("deeper");
    let recorder = TraceRecorder::new(broken_dir.join("trace.jsonl"));
    recorder
        .start_tool_span("search")
        .end(&SpanStatus::Ok, SpanOutcome::default());
    recorder.settle();
    assert!(!broken_dir.exists());
    let disabled = TraceRecorder::disabled();
    assert!(!disabled.is_enabled());
    disabled
        .start_tool_span("search")
        .end(&SpanStatus::Ok, SpanOutcome::default());
    disabled.settle();
}

#[test]
fn the_cap_keeps_the_newest_half_and_a_torn_tail_is_tolerated() {
    let dir = tempfile::tempdir().expect("state");
    let path = dir.path().join("trace.jsonl");
    // Fill to within 100 bytes of the cap, so the next record breaches it.
    let payload = "x".repeat(1024);
    let mut filler = Vec::new();
    let mut size = 0_u64;
    let mut index = 0;
    while size < TraceRecorder::LOG_CAP_BYTES - 100 {
        let line = serde_json::json!({ "seq": index, "payload": payload }).to_string();
        size += line.len() as u64 + 1;
        filler.push(line);
        index += 1;
    }
    std::fs::write(&path, format!("{}\n", filler.join("\n"))).expect("fill");

    let recorder = TraceRecorder::new(&path);
    recorder
        .start_tool_span("cap-probe")
        .end(&SpanStatus::Ok, SpanOutcome::default());
    recorder.settle();
    let text = std::fs::read_to_string(&path).expect("trace");
    assert!(text.len() as u64 <= TraceRecorder::LOG_CAP_BYTES);
    let lines: Vec<&str> = text.split('\n').filter(|line| !line.is_empty()).collect();
    assert_eq!(lines.len(), filler.len() / 2 + 1);
    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("json");
    assert_eq!(first["seq"], filler.len().div_ceil(2));
    assert!(lines.last().expect("record").contains("\"tool:cap-probe\""));

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append");
    file.write_all(b"{\"v\":1,\"truncated-mid-")
        .expect("torn tail");
    drop(file);
    recorder
        .start_tool_span("merged-into-tear")
        .end(&SpanStatus::Ok, SpanOutcome::default());
    recorder
        .start_tool_span("after-tear")
        .end(&SpanStatus::Ok, SpanOutcome::default());
    recorder.settle();
    let records = read_trace(&path);
    assert!(
        records
            .iter()
            .any(|record| record["name"] == "tool:after-tear")
    );
    assert!(
        !records
            .iter()
            .any(|record| record["name"] == "tool:merged-into-tear")
    );
}

#[test]
fn every_line_passes_the_credential_redactor() {
    let dir = tempfile::tempdir().expect("state");
    let path = dir.path().join("trace.jsonl");
    let recorder = TraceRecorder::new(&path);
    let span = recorder.start_tool_span("search");
    span.engine_child("lookup ghp_0123456789abcdefghijklmnopqrstuvwxyz0123")
        .end(&SpanStatus::Ok);
    span.end(&SpanStatus::Ok, SpanOutcome::default());
    recorder.settle();
    let raw = std::fs::read_to_string(&path).expect("trace");
    assert!(raw.contains("[REDACTED:github-token]"));
    assert!(!raw.contains("ghp_0123456789"));
}
