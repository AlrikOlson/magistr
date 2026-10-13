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
