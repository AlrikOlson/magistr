use super::*;
use tempfile::TempDir;

#[test]
fn tracer_writes_ndjson() {
    let tmp = TempDir::new().unwrap();
    let tracer = InterviewTracer::new(tmp.path());

    tracer.log_session_start("claude-sonnet", 20, &["Read".into()], 5000);
    tracer.log_assistant_text("Hello, I'll analyze your project.");
    tracer.log_tool_use("magistr_ask", "tool_123", &json!({"category": "testing"}));
    tracer.log_followup_sent("Pedantic");
    tracer.log_session_complete("all categories answered");

    let content = std::fs::read_to_string(tracer.log_path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 5, "Expected 5 trace lines");

    // Each line should be valid JSON
    for line in &lines {
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(parsed.get("ts").is_some());
        assert!(parsed.get("elapsed_ms").is_some());
        assert!(parsed.get("event").is_some());
        assert!(parsed.get("data").is_some());
    }

    // Check event types
    let events: Vec<String> = lines
        .iter()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            v["event"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(
        events,
        vec![
            "session_start",
            "assistant_text",
            "tool_use",
            "followup_sent",
            "session_complete"
        ]
    );
}

#[test]
fn tracer_noop_on_invalid_path() {
    // Should not panic even if the directory cannot be created: a path
    // under a *file* is impossible everywhere (a path under `/` is
    // creatable on Windows, where `/` is the current drive).
    let tmp = tempfile::tempdir().expect("tempdir");
    let file = tmp.path().join("file");
    std::fs::write(&file, "").expect("write");
    let tracer = InterviewTracer::new(&file.join("that/wont/work"));
    tracer.log_session_start("test", 1, &[], 0);
    // No assertions needed — just verifying it doesn't panic
}

#[test]
fn truncate_long_string() {
    let long = "a".repeat(1000);
    let short = truncate(&long, 100);
    // 99 ASCII chars + "…" (3 bytes UTF-8)
    assert_eq!(short.len(), 102);
    assert!(short.ends_with('…'));
}

#[test]
fn truncate_short_string() {
    let short = "hello";
    assert_eq!(truncate(short, 100), "hello");
}
