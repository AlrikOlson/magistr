use super::*;

use rmcp::handler::server::wrapper::Parameters;

use crate::request::RequestService;
use crate::request::domain::RequestStatus;
use crate::request::mcp::handlers::{CaptureArgs, ResearchArgs};

fn application() -> RequestUseCases {
    RequestUseCases::new(Arc::new(Mutex::new(RequestEngine::new("project".into()))))
}

#[test]
fn native_capture_and_research_enforce_validation_before_mutation() {
    let app = application();
    let error = app
        .capture(RequestKind::Bug, "user".into(), "  ".into(), None)
        .unwrap_err();
    assert_eq!(
        error,
        RequestError::InvalidArgs("request body is empty".into())
    );
    assert_eq!(app.status().unwrap().counts["total"], 0);
    let bad_content = StructuredContent {
        version: 2,
        summary: "Valid words".into(),
        facts: None,
        sections: None,
    };
    assert!(matches!(
        app.capture(
            RequestKind::Suggestion,
            "user".into(),
            "Idea".into(),
            Some(bad_content)
        ),
        Err(RequestError::InvalidArgs(_))
    ));
    assert_eq!(app.status().unwrap().counts["total"], 0);
    let request = app
        .capture(RequestKind::Bug, "user".into(), "Broken view".into(), None)
        .unwrap();
    assert!(matches!(
        app.research(&request.value.id, "\n".into(), 0.9, vec![], None),
        Err(RequestError::InvalidArgs(_))
    ));
    assert_eq!(app.get(&request.value.id).unwrap(), request.value);
    let updated = app
        .research(&request.value.id, "Confirmed".into(), 2.0, vec![], Some(42))
        .unwrap();
    assert_eq!(updated.value.status, RequestStatus::Researched);
    assert_eq!(updated.value.enrichment[0].confidence, 1.0);
    assert_eq!(updated.value.refs, ["decision:42"]);
    assert_eq!(app.ready(None, &[], None).unwrap(), [updated.value]);
}

#[tokio::test]
async fn mcp_and_native_views_share_state_and_logical_error_classification() {
    let app = application();
    let service = RequestService::from_use_cases(app.clone());
    let result = service
        .request_capture(Parameters(CaptureArgs {
            kind: "suggestion".into(),
            from: "user".into(),
            body: "  ".into(),
            content: None,
        }))
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    let error = result.structured_content.unwrap();
    let native = app
        .capture(RequestKind::Suggestion, "user".into(), "  ".into(), None)
        .unwrap_err();
    assert_eq!(error["error_kind"], native.kind());
    assert_eq!(error["message"], native.to_string());
    let request = app
        .capture(
            RequestKind::Suggestion,
            "user".into(),
            "A useful idea".into(),
            None,
        )
        .unwrap();
    let result = service
        .request_research(Parameters(ResearchArgs {
            id: request.value.id.clone(),
            summary: "Evidence".into(),
            confidence: 0.8,
            sources: vec!["local source".into()],
            note: Some(9),
        }))
        .await
        .unwrap();
    assert_eq!(
        result.structured_content.unwrap()["value"],
        serde_json::to_value(app.get(&request.value.id).unwrap()).unwrap()
    );
    assert!(Arc::ptr_eq(&service.engine(), &app.engine()));
}

#[test]
fn native_link_and_terminal_mutations_keep_domain_rules() {
    let app = application();
    let request = app
        .capture(RequestKind::Question, "user".into(), "Why?".into(), None)
        .unwrap();
    assert!(matches!(
        app.link(&request.value.id, "unknown:value"),
        Err(RequestError::InvalidArgs(_))
    ));
    app.link(&request.value.id, "decision:8").unwrap();
    assert_eq!(
        app.link(&request.value.id, "decision:8")
            .unwrap()
            .value
            .refs,
        ["decision:8"]
    );
    assert!(
        app.snooze(&request.value.id, None)
            .unwrap()
            .value
            .snooze_until
            .is_some()
    );
    assert_eq!(
        app.dismiss(&request.value.id).unwrap().value.status,
        RequestStatus::Dismissed
    );
    assert!(matches!(
        app.research(&request.value.id, "Later".into(), 1.0, vec![], None),
        Err(RequestError::InvalidState(_))
    ));
}
