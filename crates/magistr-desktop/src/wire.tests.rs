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
