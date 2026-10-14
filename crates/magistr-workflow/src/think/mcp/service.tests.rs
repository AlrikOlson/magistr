use super::*;

use rmcp::handler::server::wrapper::Parameters;

use crate::think::config::ThinkConfig;
use crate::think::mcp::args::{
    BranchStatusArgs, ExportArgs, ImpactArgs, NoArgs, PinArgs, ReviseEstimateArgs,
};

fn service() -> ThinkService {
    let mut config = ThinkConfig::default();
    config.display.color_output = false;
    ThinkService::new(ReasoningServer::new_for_project(
        config,
        "isolated-poison-parity".into(),
    ))
}

#[tokio::test]
async fn poisoned_native_owner_remains_an_internal_mcp_error() {
    let service = service();
    let peer = ThinkService::from_use_cases(service.use_cases());
    let engine = service.engine();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = engine.lock().unwrap();
        panic!("isolated native owner failure");
    }));
    assert!(panic.is_err());
    assert!(engine.is_poisoned());

    // Exercise each presentation branch affected by native extraction. The
    // second MCP view shares the poisoned owner without opening another one.
    let results = [
        peer.think_record_step(Parameters(
            serde_json::from_value(serde_json::json!({})).unwrap(),
        ))
        .await,
        peer.think_export_trace(Parameters(ExportArgs::default()))
            .await,
        peer.think_step_impact(Parameters(ImpactArgs { step_number: 1 }))
            .await,
        peer.think_pin_step(Parameters(PinArgs {
            step_number: 1,
            pinned: Some(true),
        }))
        .await,
        peer.think_revise_estimate(Parameters(ReviseEstimateArgs {
            estimated_total: 1,
            reason: None,
        }))
        .await,
        peer.think_set_branch_status(Parameters(BranchStatusArgs {
            branch_id: "branch".into(),
            status: "active".into(),
            merged_into: None,
        }))
        .await,
        peer.think_trace_checkpoint(Parameters(NoArgs {})).await,
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
        .think_step_impact(Parameters(ImpactArgs { step_number: 1 }))
        .await
        .expect("missing step is a tool result");
    assert_eq!(missing.is_error, Some(true));
    assert_eq!(
        missing.structured_content.unwrap()["error_kind"],
        "step_not_found"
    );

    let invalid_export = service
        .think_export_trace(Parameters(ExportArgs {
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

    let invalid_step = service
        .think_record_step(Parameters(
            serde_json::from_value(serde_json::json!({})).unwrap(),
        ))
        .await
        .expect("invalid step is a tool result");
    assert_eq!(invalid_step.is_error, Some(true));
    let failure = invalid_step.structured_content.unwrap();
    assert_eq!(failure["status"], "failed");
    assert!(
        failure["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty())
    );
    assert!(service.use_cases().get_step(1, false).unwrap().is_none());
}

fn valid_step() -> crate::think::domain::ThinkStep {
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
        .think_record_step(Parameters(valid_step()))
        .await
        .unwrap();
    let body = result.structured_content.unwrap();
    assert_eq!(body["source"]["kind"], "memory");
    assert!(body["record_id"].as_str().is_some_and(|id| !id.is_empty()));
}

#[tokio::test]
async fn mcp_reports_the_owner_write_refusal_and_retry_saves_once() {
    let directory = tempfile::tempdir().unwrap();
    let mut config = ThinkConfig::default();
    config.persistence.enabled = true;
    config.persistence.data_dir = directory.path().to_path_buf();
    let persistence =
        crate::think::persistence::Persistence::for_project(&config.persistence, "mcp-write");
    let service = ThinkService::new(ReasoningServer::new_for_project(config, "mcp-write".into()));
    let first = service
        .think_record_step(Parameters(valid_step()))
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
            .think_record_step(Parameters(valid_step()))
            .await
            .unwrap(),
        service
            .think_pin_step(Parameters(PinArgs {
                step_number: 1,
                pinned: Some(true),
            }))
            .await
            .unwrap(),
        service
            .think_revise_estimate(Parameters(ReviseEstimateArgs {
                estimated_total: 20,
                reason: None,
            }))
            .await
            .unwrap(),
        service
            .think_wipe_trace(Parameters(NoArgs {}))
            .await
            .unwrap(),
    ];
    for failed in failures {
        assert_eq!(failed.is_error, Some(true));
        let body = failed.structured_content.unwrap();
        assert_eq!(body["effect"], "none");
        assert_eq!(body["code"], "write_failed");
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(service.engine().lock().unwrap().history().steps.len(), 1);
    std::fs::remove_dir(lock).unwrap();
    let retry = service
        .think_record_step(Parameters(valid_step()))
        .await
        .unwrap();
    let body = retry.structured_content.unwrap();
    assert_eq!(body["step_number"], 2);
    assert_eq!(body["source"]["kind"], "file");
    assert_eq!(
        persistence
            .load_default()
            .unwrap()
            .value
            .unwrap()
            .steps
            .len(),
        2
    );
}

#[tokio::test]
async fn branch_status_acknowledges_the_committed_merge_destination() {
    let service = service();
    service.use_cases().record_step(valid_step()).unwrap();
    let mut branch = valid_step();
    branch.branch_from = Some(1);
    branch.branch_name = Some("Does the evidence agree?".into());
    let branch_id = service
        .use_cases()
        .record_step(branch)
        .unwrap()
        .step
        .branch_id
        .unwrap();
    service
        .use_cases()
        .set_branch_status(&branch_id, "merged", Some(2))
        .unwrap();
    let result = service
        .think_set_branch_status(Parameters(BranchStatusArgs {
            branch_id,
            status: "merged".into(),
            merged_into: None,
        }))
        .await
        .unwrap();
    assert_eq!(result.structured_content.unwrap()["merged_into"], 2);
}
