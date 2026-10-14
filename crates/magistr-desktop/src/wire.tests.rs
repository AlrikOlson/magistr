use super::*;

#[test]
fn turn_outcomes_remain_distinct() {
    assert_eq!(ended(TurnEnd::Cancelled), "Stopped");
    assert_eq!(ended(TurnEnd::Refusal), "Declined");
    assert_eq!(ended(TurnEnd::EndTurn), "Finished");
}
#[test]
fn authored_text_is_exact_and_internal_protocol_events_are_not_copy() {
    let text = "日本\n<command>";
    let json = serde_json::to_value(observation(&Msg::InputAccepted(text.into()))).unwrap();
    assert_eq!(json["data"], text);
    assert_eq!(json["kind"], "user");
    assert!(
        observation(&Msg::Agent(AgentEvent::Terminal {
            terminal_id: "t".into(),
            bytes: vec![27]
        }))
        .is_none()
    );
}

#[test]
fn saved_spending_is_visible_without_inventing_context_counts() {
    let usage = serde_json::to_value(Usage::from_report(None, Some(25))).unwrap();
    assert!(usage["used"].is_null());
    assert!(usage["size"].is_null());
    assert_eq!(usage["cost_cents"], 25);
}

#[test]
fn reported_zero_context_and_unknown_spending_stay_distinct() {
    let usage = serde_json::to_value(Usage::from_report(
        Some(magistr_application::sessions::ContextUsage { used: 0, size: 100 }),
        None,
    ))
    .unwrap();
    assert_eq!(usage["used"], 0);
    assert_eq!(usage["size"], 100);
    assert!(usage["cost_cents"].is_null());
}
#[test]
fn catalog_frames_preserve_full_ownership_and_omit_transcript_and_request_details() {
    let summary = magistr::application::SessionSummary {
        address: SessionAddress {
            project: "project".into(),
            workspace: "copy".into(),
            session: "resident".into(),
        },
        name: "Project".into(),
        conversation: None,
        status: magistr::application::SessionStatus::Working,
        closed: false,
        model: None,
        usage: Some(Msg::Agent(AgentEvent::Usage {
            context: None,
            cost_cents: Some(0),
            elapsed_secs: None,
        })),
        requests: vec![magistr::application::WaitingRequest {
            id: 7,
            title: "Read".into(),
        }],
        activity: Some(magistr_application::sessions::SessionActivity {
            active: true,
            queued_turns: 2,
            configuration_in_progress: false,
            cancellation_requested: false,
            can_resume: Some(false),
            can_send_images: None,
        }),
    };
    let frame = CatalogFrame::from_catalog(
        9,
        &SessionCatalog {
            sequence: 42,
            sessions: vec![summary],
        },
    );
    let json = serde_json::to_value(frame).unwrap();
    assert_eq!(json["stream"], 9);
    assert_eq!(json["sequence"], 42);
    let session = &json["sessions"][0];
    assert_eq!(session["address"]["project"], "project");
    assert_eq!(session["address"]["workspace"], "copy");
    assert_eq!(session["address"]["session"], "resident");
    assert_eq!(session["status"], "Waiting for you");
    assert_eq!(session["pending"][0]["id"], 7);
    assert!(session["pending"][0].get("detail").is_none());
    assert!(session.get("events").is_none());
    assert_eq!(session["usage"]["cost_cents"], 0);
    assert!(session["usage"]["used"].is_null());
    assert_eq!(session["activity"]["can_resume"], false);
    assert!(session["activity"]["can_send_images"].is_null());
}
#[test]
fn admissions_preserve_the_real_cursor_and_distinguish_refusal_from_acceptance() {
    let accepted = admission(4, &Msg::InputAccepted("hello".into())).unwrap();
    let rejected = admission(
        5,
        &Msg::Agent(AgentEvent::PromptRejected {
            text: "keep me".into(),
            error: "full".into(),
        }),
    )
    .unwrap();
    let value = serde_json::to_value(accepted).unwrap();
    assert_eq!(value["sequence"], 4);
    assert_eq!(value["text"], "hello");
    assert_eq!(value["accepted"], true);
    assert!(!rejected.accepted);
}
