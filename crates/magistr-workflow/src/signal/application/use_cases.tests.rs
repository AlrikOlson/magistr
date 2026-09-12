use super::*;

use rmcp::handler::server::wrapper::Parameters;

use crate::signal::SignalService;
use crate::signal::domain::SignalStatus;
use crate::signal::mcp::handlers::{CaptureArgs, ResearchArgs};

fn application() -> SignalUseCases {
    SignalUseCases::new(Arc::new(Mutex::new(SignalEngine::new("project".into()))))
}

#[test]
fn native_capture_and_research_enforce_validation_before_mutation() {
    let app = application();
    let error = app
        .capture(SignalKind::Bug, "user".into(), "  ".into(), None)
        .unwrap_err();
    assert_eq!(
        error,
        SignalError::InvalidArgs("signal body is empty".into())
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
            SignalKind::Idea,
            "user".into(),
            "Idea".into(),
            Some(bad_content)
        ),
        Err(SignalError::InvalidArgs(_))
    ));
    assert_eq!(app.status().unwrap().counts["total"], 0);
    let signal = app
        .capture(SignalKind::Bug, "user".into(), "Broken view".into(), None)
        .unwrap();
    assert!(matches!(
        app.research(&signal.id, "\n".into(), 0.9, vec![], None),
        Err(SignalError::InvalidArgs(_))
    ));
    assert_eq!(app.get(&signal.id).unwrap(), signal);
    let updated = app
        .research(&signal.id, "Confirmed".into(), 2.0, vec![], Some(42))
        .unwrap();
    assert_eq!(updated.status, SignalStatus::Researched);
    assert_eq!(updated.enrichment[0].confidence, 1.0);
    assert_eq!(updated.cross_refs, ["think:42"]);
    assert_eq!(app.pending(None, &[], None).unwrap(), [updated]);
}

#[tokio::test]
async fn mcp_and_native_views_share_state_and_logical_error_classification() {
    let app = application();
    let service = SignalService::from_use_cases(app.clone());
    let result = service
        .signal_capture(Parameters(CaptureArgs {
            kind: "idea".into(),
            from: "user".into(),
            body: "  ".into(),
            content: None,
        }))
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    let error = result.structured_content.unwrap();
    let native = app
        .capture(SignalKind::Idea, "user".into(), "  ".into(), None)
        .unwrap_err();
    assert_eq!(error["error_kind"], native.kind());
    assert_eq!(error["message"], native.to_string());
    let signal = app
        .capture(
            SignalKind::Idea,
            "user".into(),
            "A useful idea".into(),
            None,
        )
        .unwrap();
    let result = service
        .signal_research(Parameters(ResearchArgs {
            id: signal.id.clone(),
            summary: "Evidence".into(),
            confidence: 0.8,
            sources: vec!["local source".into()],
            think_step: Some(9),
        }))
        .await
        .unwrap();
    assert_eq!(
        result.structured_content.unwrap(),
        serde_json::to_value(app.get(&signal.id).unwrap()).unwrap()
    );
    assert!(Arc::ptr_eq(&service.engine(), &app.engine()));
}

#[test]
fn native_link_and_terminal_mutations_keep_domain_rules() {
    let app = application();
    let signal = app
        .capture(SignalKind::Question, "user".into(), "Why?".into(), None)
        .unwrap();
    assert!(matches!(
        app.link(&signal.id, "unknown:value"),
        Err(SignalError::InvalidArgs(_))
    ));
    app.link(&signal.id, "think:8").unwrap();
    assert_eq!(
        app.link(&signal.id, "think:8").unwrap().cross_refs,
        ["think:8"]
    );
    assert!(app.snooze(&signal.id, None).unwrap().snooze_until.is_some());
    assert_eq!(
        app.ignore(&signal.id).unwrap().status,
        SignalStatus::Dismissed
    );
    assert!(matches!(
        app.research(&signal.id, "Later".into(), 1.0, vec![], None),
        Err(SignalError::InvalidState(_))
    ));
}
