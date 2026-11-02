use super::*;

use rmcp::handler::server::wrapper::Parameters;

use crate::decision::config::DecisionConfig;
use crate::decision::mcp::args::{
    ExportArgs, ImpactArgs, NoArgs, PinArgs, ReviseEstimateArgs, ThreadStatusArgs,
};

fn service() -> DecisionService {
    let mut config = DecisionConfig::default();
    config.display.color_output = false;
    DecisionService::new(ReasoningServer::new_for_project(
        config,
        "isolated-poison-parity".into(),
    ))
}

#[tokio::test]
async fn poisoned_native_owner_remains_an_internal_mcp_error() {
    let service = service();
    let peer = DecisionService::from_use_cases(service.use_cases());
    let engine = service.engine();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = engine.lock().unwrap();
        panic!("isolated native owner failure");
    }));
    assert!(panic.is_err());
    assert!(engine.is_poisoned());

    // Exercise each presentation thread affected by native extraction. The
    // second MCP view shares the poisoned owner without opening another one.
    let results = [
        peer.decision_record(Parameters(
            serde_json::from_value(serde_json::json!({})).unwrap(),
        ))
        .await,
        peer.decision_export(Parameters(ExportArgs::default()))
            .await,
        peer.decision_impact(Parameters(ImpactArgs { note: 1 }))
            .await,
        peer.decision_pin(Parameters(PinArgs {
            note: 1,
            pinned: Some(true),
        }))
        .await,
        peer.decision_revise_estimate(Parameters(ReviseEstimateArgs {
            estimated_total: 1,
            reason: None,
        }))
        .await,
        peer.decision_set_thread_status(Parameters(ThreadStatusArgs {
            thread_id: "thread".into(),
            status: "open".into(),
            merged_into: None,
        }))
        .await,
        peer.decision_review(Parameters(Default::default())).await,
    ];
    for result in results {
        let error = result.expect_err("owner failure must remain a JSON-RPC internal error");
        let expected = ErrorData::internal_error("engine mutex poisoned", None);
        assert_eq!(error.code, expected.code);
        assert_eq!(error.message, expected.message);
        assert_eq!(error.data, expected.data);
    }
}

#[tokio::test]
async fn native_domain_refusals_keep_their_existing_tool_error_shapes() {
    let service = service();
    let missing = service
        .decision_impact(Parameters(ImpactArgs { note: 1 }))
        .await
        .expect("missing note is a tool result");
    assert_eq!(missing.is_error, Some(true));
    assert_eq!(
        missing.structured_content.unwrap()["error_kind"],
        "note_not_found"
    );

    let invalid_export = service
        .decision_export(Parameters(ExportArgs {
            log_id: None,
            format: Some("unknown-format".into()),
        }))
        .await
        .expect("unsupported format is a tool result");
    assert_eq!(invalid_export.is_error, Some(true));
    assert!(invalid_export.content.iter().any(|content| {
        content
            .as_text()
            .is_some_and(|text| text.text.contains("Unknown format"))
    }));

    let invalid_note = service
        .decision_record(Parameters(
            serde_json::from_value(serde_json::json!({})).unwrap(),
        ))
        .await
        .expect("invalid note is a tool result");
    assert_eq!(invalid_note.is_error, Some(true));
    let failure = invalid_note.structured_content.unwrap();
    assert_eq!(failure["status"], "failed");
    assert!(
        failure["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty())
    );
    assert!(service.use_cases().get_note(1, false).unwrap().is_none());
}

fn valid_note() -> crate::decision::domain::Note {
    serde_json::from_value(serde_json::json!({
        "estimated_total": 8, "purpose": "analysis", "context": "durability",
        "thought": "The saved record must exist before the acknowledgement",
        "outcome": "checked", "next_action": "continue", "rationale": "verify"
    }))
    .unwrap()
}

#[tokio::test]
async fn memory_only_acknowledgement_is_explicit() {
    let result = service()
        .decision_record(Parameters(valid_note()))
        .await
        .unwrap();
    let body = result.structured_content.unwrap();
    assert_eq!(body["source"]["kind"], "memory");
    assert!(body["record_id"].as_str().is_some_and(|id| !id.is_empty()));
}

#[tokio::test]
async fn mcp_reports_the_owner_write_refusal_and_retry_saves_once() {
    let directory = tempfile::tempdir().unwrap();
    let mut config = DecisionConfig::default();
    config.persistence.enabled = true;
    config.persistence.data_dir = directory.path().to_path_buf();
    let persistence =
        crate::decision::persistence::Persistence::for_project(&config.persistence, "mcp-write");
    let service =
        DecisionService::new(ReasoningServer::new_for_project(config, "mcp-write".into()));
    let first = service
        .decision_record(Parameters(valid_note()))
        .await
        .unwrap();
    assert_eq!(first.structured_content.unwrap()["source"]["kind"], "file");
    let path = persistence.store_path().unwrap();
    let original = std::fs::read(&path).unwrap();
    let lock = path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let failures = [
        service
            .decision_record(Parameters(valid_note()))
            .await
            .unwrap(),
        service
            .decision_pin(Parameters(PinArgs {
                note: 1,
                pinned: Some(true),
            }))
            .await
            .unwrap(),
        service
            .decision_revise_estimate(Parameters(ReviseEstimateArgs {
                estimated_total: 20,
                reason: None,
            }))
            .await
            .unwrap(),
        service.decision_wipe(Parameters(NoArgs {})).await.unwrap(),
    ];
    for failed in failures {
        assert_eq!(failed.is_error, Some(true));
        let body = failed.structured_content.unwrap();
        assert_eq!(body["effect"], "none");
        assert_eq!(body["code"], "write_failed");
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(service.engine().lock().unwrap().history().notes.len(), 1);
    std::fs::remove_dir(lock).unwrap();
    let retry = service
        .decision_record(Parameters(valid_note()))
        .await
        .unwrap();
    let body = retry.structured_content.unwrap();
    assert_eq!(body["note"], 2);
    assert_eq!(body["source"]["kind"], "file");
    assert_eq!(
        persistence
            .load_default()
            .unwrap()
            .value
            .unwrap()
            .notes
            .len(),
        2
    );
}

#[tokio::test]
async fn thread_status_acknowledges_the_committed_merge_destination() {
    let service = service();
    service.use_cases().record(valid_note()).unwrap();
    let mut thread = valid_note();
    thread.thread_from = Some(1);
    thread.thread_name = Some("Does the evidence agree?".into());
    let thread_id = service
        .use_cases()
        .record(thread)
        .unwrap()
        .note
        .thread_id
        .unwrap();
    service
        .use_cases()
        .set_thread_status(&thread_id, "merged", Some(2))
        .unwrap();
    let result = service
        .decision_set_thread_status(Parameters(ThreadStatusArgs {
            thread_id,
            status: "merged".into(),
            merged_into: None,
        }))
        .await
        .unwrap();
    assert_eq!(result.structured_content.unwrap()["merged_into"], 2);
}
