use std::time::Duration;

use super::*;

#[test]
fn deltas_are_spaced_by_the_gap_and_the_turn_ends_last() {
    let agent = FakeAgent::new()
        .turn_ended(Duration::from_millis(50))
        .deltas(Duration::from_millis(10), 3, Duration::from_millis(5), "x")
        .tool_call(Duration::from_millis(30), "tc-1", "Read");
    assert_eq!(agent.len(), 5);
    assert!(!agent.is_empty());
    let steps = agent.into_timeline().into_steps();
    let times: Vec<u64> = steps.iter().map(|(at, _)| at.as_millis() as u64).collect();
    assert_eq!(times, vec![10, 15, 20, 30, 50]);
    assert!(matches!(
        steps[3].1,
        Msg::Agent(AgentEvent::ToolCall { ref id, status: ToolStatus::InProgress, .. }) if id == "tc-1"
    ));
    assert_eq!(
        steps[4].1,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn))
    );
}

#[tokio::test]
async fn plays_onto_the_inbox() {
    let (inbox, mut rx) = Inbox::channel();
    FakeAgent::new()
        .thought(Duration::ZERO, "hmm")
        .text(Duration::ZERO, "hi")
        .play(inbox);
    assert_eq!(
        rx.recv().await,
        Some(Msg::Agent(AgentEvent::Thought("hmm".to_owned())))
    );
    assert_eq!(
        rx.recv().await,
        Some(Msg::Agent(AgentEvent::Text("hi".to_owned())))
    );
}
