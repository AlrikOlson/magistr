//! 2026-style MCP wire-shape tests for `DecisionService`.
//!
//! Asserts the tools/list and tools/call surface meet the post-2025-06-18
//! spec expectations: every tool carries `ToolAnnotations`, JSON-returning
//! tools advertise `output_schema`, and call results emit
//! `structured_content` so 2026 clients can validate and pattern-match
//! without parsing prose.

use std::collections::BTreeSet;

use magistr_workflow::decision::config::DecisionConfig;
use magistr_workflow::decision::domain::{NextAction, Note};
use magistr_workflow::decision::engine::core::ReasoningServer;
use magistr_workflow::decision::mcp::args::{
    ExportArgs, ImpactArgs, LogArgs, NoArgs, NoteLookupArgs, PinArgs, ReviseEstimateArgs,
    SearchArgs,
};
use magistr_workflow::decision::mcp::service::DecisionService;
use magistr_workflow::decision::output_schemas::output_schema_for;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;

fn svc() -> DecisionService {
    let mut cfg = DecisionConfig::default();
    cfg.display.color_output = false;
    DecisionService::new(ReasoningServer::new(cfg))
}

fn base_note(n: u32) -> Note {
    Note {
        note: n,
        estimated_total: 3,
        purpose: "analysis".into(),
        context: "Test context".into(),
        thought: "Test thought".into(),
        outcome: "Test outcome".into(),
        next_action: NextAction::Text("Test next action".into()),
        rationale: "Test rationale".into(),
        confidence: None,
        uncertainty_notes: None,
        revises_note: None,
        revision_reason: None,
        revised_by: None,
        is_final_note: None,
        thread_from: None,
        thread_id: None,
        thread_name: None,
        tools_used: None,
        dependencies: None,
        timestamp: None,
        duration_ms: None,
        log_id: None,
        pinned: None,
        cwd: None,
        run_ref: None,
        record_id: None,
    }
}

fn structured(result: &CallToolResult) -> &serde_json::Value {
    result
        .structured_content
        .as_ref()
        .expect("expected structured_content on the CallToolResult")
}

#[tokio::test]
async fn typed_reasoning_and_mcp_share_identity_and_validation() {
    let engine = std::sync::Arc::new(std::sync::Mutex::new(ReasoningServer::new_for_project(
        DecisionConfig::default(),
        "typed-project".into(),
    )));
    let application = magistr_workflow::decision::application::DecisionUseCases::new(engine);
    let wire = DecisionService::from_use_cases(application.clone());
    let recorded = application.record(base_note(0)).expect("native note");
    assert_eq!(recorded.project_id, "typed-project");
    assert_eq!(recorded.note.note, 1);
    assert!(recorded.note.record_id.is_some());
    let found = wire
        .decision_get(Parameters(NoteLookupArgs {
            note: recorded.note.note,
            resolve_latest: None,
        }))
        .await
        .expect("wire read");
    assert_eq!(
        structured(&found)["record_id"],
        recorded.note.record_id.expect("identity")
    );
    wire.decision_pin(Parameters(PinArgs {
        note: 1,
        pinned: Some(true),
    }))
    .await
    .expect("wire pin");
    assert_eq!(
        application
            .get_note(1, false)
            .expect("native read")
            .expect("note")
            .pinned,
        Some(true)
    );

    let mut invalid = base_note(0);
    invalid.rationale.clear();
    let native_error = application
        .record(invalid.clone())
        .expect_err("native refusal");
    let wire_error = wire
        .decision_record(Parameters(invalid))
        .await
        .expect("wire refusal");
    assert_eq!(structured(&wire_error)["error"], native_error.error);
    assert!(application.get_note(2, false).expect("read").is_none());
}

// ─── Tool list shape ────────────────────────────────────────────────────

const EXPECTED_TOOL_NAMES: &[&str] = &[
    "decision_record",
    "decision_status",
    "decision_export",
    "decision_get",
    "decision_search",
    "decision_impact",
    "decision_pin",
    "decision_revise_estimate",
    "decision_set_thread_status",
    "decision_review",
    "decision_wipe",
    "decision_logs",
    "decision_log",
];

#[test]
fn tools_list_has_exactly_the_decision_canonicals() {
    let s = svc();
    let names: BTreeSet<String> = s
        .list_tools_view()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    // Set EQUALITY, not containment: any name this family stops serving, and
    // any name it starts serving without being told to, both fail here. That
    // subsumes checking a hand-maintained list of names it must not serve.
    let expected: BTreeSet<String> = EXPECTED_TOOL_NAMES.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        names, expected,
        "tool list should expose exactly the 13 decision_* canonical names"
    );
}

#[test]
fn tools_list_carries_annotations() {
    let s = svc();
    for tool in s.list_tools_view() {
        let ann = tool
            .annotations
            .unwrap_or_else(|| panic!("tool {:?} should carry ToolAnnotations", tool.name));
        // Every tool gets a human-readable title.
        assert!(
            ann.title.is_some(),
            "tool {:?} missing annotations.title",
            tool.name
        );
        // Every tool declares all four hint booleans. (Some may be None,
        // but we set all four on every tool, so check at least the
        // open_world_hint = false invariant — none of our tools touch
        // external systems.)
        assert_eq!(
            ann.open_world_hint,
            Some(false),
            "tool {:?} should have openWorldHint=false (engine-local)",
            tool.name
        );
    }
}

#[test]
fn tools_list_carries_hints() {
    let s = svc();
    let tools: std::collections::HashMap<String, _> = s
        .list_tools_view()
        .into_iter()
        .map(|t| (t.name.to_string(), t))
        .collect();

    // decision_status: read-only, idempotent.
    let status = tools.get("decision_status").unwrap();
    let ann = status.annotations.as_ref().unwrap();
    assert_eq!(ann.read_only_hint, Some(true));
    assert_eq!(ann.destructive_hint, Some(false));
    assert_eq!(ann.idempotent_hint, Some(true));

    // decision_record: mutates, not destructive, not idempotent.
    let record = tools.get("decision_record").unwrap();
    let ann = record.annotations.as_ref().unwrap();
    assert_eq!(ann.read_only_hint, Some(false));
    assert_eq!(ann.destructive_hint, Some(false));
    assert_eq!(ann.idempotent_hint, Some(false));

    // decision_wipe: destructive — the load-bearing hint.
    let wipe = tools.get("decision_wipe").unwrap();
    let ann = wipe.annotations.as_ref().unwrap();
    assert_eq!(
        ann.destructive_hint,
        Some(true),
        "wipe must request destructive=true for client confirmation gating"
    );
}

#[test]
fn tools_list_attaches_output_schema_to_json_returning_tools() {
    let s = svc();
    let tools = s.list_tools_view();
    for tool in &tools {
        let expects_schema = output_schema_for(&tool.name).is_some();
        if expects_schema {
            assert!(
                tool.output_schema.is_some(),
                "tool {:?} should have output_schema attached",
                tool.name
            );
        }
    }
    // Spot-check: decision_export returns format-dependent text,
    // so it intentionally has no output_schema.
    let exp = tools.iter().find(|t| t.name == "decision_export").unwrap();
    assert!(
        exp.output_schema.is_none(),
        "decision_export returns text — should NOT have output_schema"
    );
}

#[test]
fn tool_descriptions_carry_pitfalls_marker() {
    // arXiv:2602.14878 (Feb 2026) found descriptions with explicit
    // pitfall/gotcha sections score highest on agent selection accuracy.
    // Every tool in our surface should advertise its pitfalls.
    let s = svc();
    for tool in s.list_tools_view() {
        let desc = tool
            .description
            .as_deref()
            .unwrap_or_else(|| panic!("tool {:?} missing description", tool.name));
        assert!(
            desc.to_ascii_lowercase().contains("pitfall"),
            "tool {:?} description missing 'Pitfall' section: {desc}",
            tool.name
        );
    }
}

// ─── Call results carry structured_content ──────────────────────────────

#[tokio::test]
async fn record_call_returns_structured_content() {
    let s = svc();
    let result = s
        .decision_record(Parameters(base_note(1)))
        .await
        .expect("record should not return ErrorData");
    assert_eq!(result.is_error, Some(false).or(None));
    let sc = structured(&result);
    assert_eq!(sc["note"], 1);
    assert_eq!(sc["estimated_total"], 3);
    assert_eq!(sc["total_notes"], 1);
}

// `engine_status_call_returns_structured_content` used to live here. It called
// the handler directly, and the handler now takes the request's own
// `RequestContext` so it can report the live client (plan task
// `mcp-stderr-observability-gap`) — which is not constructible without a wire.
// The claim was not weakened, it MOVED: `tests/client_capability_view.rs`
// asserts the same version/persistence/logs_enabled fields against a real
// client over a real duplex, in
// `the_engine_fields_are_untouched_by_the_client_block`.

#[tokio::test]
async fn get_note_call_returns_structured_note() {
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s
        .decision_get(Parameters(NoteLookupArgs {
            note: 1,
            resolve_latest: None,
        }))
        .await
        .unwrap();
    let sc = structured(&result);
    assert_eq!(sc["note"], 1);
    assert_eq!(sc["purpose"], "analysis");
}

#[tokio::test]
async fn get_note_missing_emits_soft_error_envelope() {
    let s = svc();
    let result = s
        .decision_get(Parameters(NoteLookupArgs {
            note: 999,
            resolve_latest: None,
        }))
        .await
        .unwrap();
    // A logical failure must be visible to clients before success decoding.
    assert_eq!(result.is_error, Some(true));
    let sc = structured(&result);
    assert_eq!(sc["ok"], false);
    assert_eq!(sc["error_kind"], "note_not_found");
    assert!(sc["message"].as_str().unwrap().contains("999"));
}

#[tokio::test]
async fn pin_call_returns_structured() {
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s
        .decision_pin(Parameters(PinArgs {
            note: 1,
            pinned: Some(true),
        }))
        .await
        .unwrap();
    let sc = structured(&result);
    assert_eq!(sc["note"], 1);
    assert_eq!(sc["was_pinned"], false);
    assert_eq!(sc["now_pinned"], true);
}

#[tokio::test]
async fn revise_estimate_call_returns_structured() {
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s
        .decision_revise_estimate(Parameters(ReviseEstimateArgs {
            estimated_total: 7,
            reason: Some("Scope expanded".into()),
        }))
        .await
        .unwrap();
    let sc = structured(&result);
    assert_eq!(sc["previous"], 3);
    assert_eq!(sc["new_estimate"], 7);
    assert_eq!(sc["reason"], "Scope expanded");
}

#[tokio::test]
async fn impact_call_returns_structured() {
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s
        .decision_impact(Parameters(ImpactArgs { note: 1 }))
        .await
        .unwrap();
    let sc = structured(&result);
    assert_eq!(sc["note"], 1);
    assert!(sc["upstream"].is_object());
    assert!(sc["downstream"].is_object());
    assert!(sc["revision_chain"].is_array());
}

#[tokio::test]
async fn search_call_returns_structured() {
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s
        .decision_search(Parameters(SearchArgs {
            query: "thought".into(),
            limit: Some(5),
        }))
        .await
        .unwrap();
    let sc = structured(&result);
    assert_eq!(sc["query"], "thought");
    assert!(sc["matches"].is_array());
}

#[tokio::test]
async fn review_call_returns_structured() {
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s
        .decision_review(Parameters(Default::default()))
        .await
        .unwrap();
    let sc = structured(&result);
    assert!(sc["open_hypotheses"].is_array());
    assert!(sc["stale_threads"].is_array());
    assert!(sc["confidence_trend"].is_string());
}

#[tokio::test]
async fn wipe_call_returns_structured_and_clears() {
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s.decision_wipe(Parameters(NoArgs {})).await.unwrap();
    let sc = structured(&result);
    assert_eq!(sc["cleared"], true);
    // Engine should now be empty. Probed through `decision_search` rather
    // than `decision_status`: the latter now takes the request's own
    // `RequestContext` (see the note above `get_note_call_returns_structured_note`)
    // and cannot be called without a wire. A log whose only note had purpose
    // "analysis" and which now matches nothing is the same emptiness claim.
    let found = s
        .decision_search(Parameters(SearchArgs {
            query: "analysis".into(),
            limit: None,
        }))
        .await
        .unwrap();
    assert_eq!(structured(&found)["match_count"], 0);
}

#[tokio::test]
async fn export_returns_text_no_structured_content() {
    // The one tool that intentionally returns format-dependent text:
    // structuredContent must be absent so clients know to treat as text.
    let s = svc();
    s.decision_record(Parameters(base_note(1))).await.unwrap();
    let result = s
        .decision_export(Parameters(ExportArgs {
            log_id: None,
            format: Some("json".into()),
        }))
        .await
        .unwrap();
    assert!(
        result.structured_content.is_none(),
        "decision_export should return text only, not structured"
    );
    assert!(!result.content.is_empty(), "should have text content");
}

#[tokio::test]
async fn saved_log_tools_read_full_records_and_refuse_unreadable_sources() {
    use magistr_workflow::decision::application::DecisionUseCases;
    use std::sync::{Arc, Mutex};
    let dir = tempfile::tempdir().unwrap();
    let mut config = DecisionConfig::default();
    config.features.enable_logs = true;
    config.system.max_history_size = 1;
    config.persistence.enabled = true;
    config.persistence.data_dir = dir.path().into();
    config.display.color_output = false;
    let owner = Arc::new(Mutex::new(ReasoningServer::new_for_project(
        config.clone(),
        "browse".into(),
    )));
    let app = DecisionUseCases::new(owner.clone());
    let wire = DecisionService::from_use_cases(app.clone());
    let absent = wire
        .decision_log(Parameters(LogArgs::default()))
        .await
        .unwrap();
    assert!(structured(&absent)["value"].is_null());
    for n in 1..=4 {
        let mut note = base_note(n);
        note.log_id = Some(if n == 4 { "other" } else { "chosen" }.into());
        app.record(note).unwrap();
    }
    let list = wire.decision_logs(Parameters(NoArgs {})).await.unwrap();
    assert_eq!(structured(&list)["value"].as_array().unwrap().len(), 2);
    let selected = wire
        .decision_log(Parameters(LogArgs {
            log_id: Some("chosen".into()),
        }))
        .await
        .unwrap();
    let saved = &structured(&selected)["value"];
    assert_eq!(saved["notes"].as_array().unwrap().len(), 3);
    assert_eq!(saved["log_id"], "browse__chosen");
    let found = app.get_note(1, false).unwrap().unwrap();
    assert_eq!(serde_json::to_value(found).unwrap(), saved["notes"][0]);
    let matches = app.search("Test thought", 10).unwrap();
    assert_eq!(matches.len(), 4);
    let history = app.log(Some("chosen")).unwrap().value.unwrap();
    let analysis = ReasoningServer::for_analysis(history, Default::default());
    assert_eq!(analysis.search_notes("Test thought", 10).len(), 3);
    assert_eq!(matches[0]["log_id"], "browse__chosen");
    assert!(matches[0]["record_id"].is_string());
    assert_eq!(app.status(false, Some("chosen")).unwrap()["total_notes"], 3);
    assert_eq!(app.status(false, None).unwrap()["log_present"], false);
    let exported: serde_json::Value =
        serde_json::from_str(&app.export("json", Some("chosen")).unwrap()).unwrap();
    assert_eq!(exported["notes"], saved["notes"]);
    for (name, result) in [("decision_logs", &list), ("decision_log", &selected)] {
        let schema = output_schema_for(name).unwrap();
        assert!(
            jsonschema::is_valid(
                &serde_json::Value::Object((*schema).clone()),
                structured(result)
            ),
            "{name}"
        );
    }
    let store = magistr_workflow::decision::persistence::Persistence::for_project(
        &config.persistence,
        "browse",
    );
    let path = store.store_path().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"broken").unwrap();
    let failed = wire
        .decision_get(Parameters(NoteLookupArgs {
            note: 1,
            resolve_latest: None,
        }))
        .await
        .unwrap();
    assert_eq!(failed.is_error, Some(true));
    assert_eq!(structured(&failed)["error_kind"], "source_unavailable");
    let failed = wire
        .decision_log(Parameters(LogArgs {
            log_id: Some("absent".into()),
        }))
        .await
        .unwrap();
    assert_eq!(failed.is_error, Some(true));
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        app.log(Some("chosen")).unwrap().value.unwrap().notes.len(),
        3
    );
}
