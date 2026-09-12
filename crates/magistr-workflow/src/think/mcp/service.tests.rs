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
