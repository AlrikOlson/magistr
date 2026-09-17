use super::*;
use crate::agent::ToolRecord;

fn tool(id: &str, status: ToolStatus) -> AgentEvent {
    AgentEvent::ToolCall {
        id: id.to_owned(),
        title: "private_internal_name".to_owned(),
        verb: ToolVerb::Read,
        status,
        diff: None,
        images: vec![],
        record: Box::new(ToolRecord::default()),
    }
}

#[test]
fn completing_one_of_several_tools_keeps_the_other_activity() {
    let mut activity = AgentActivity::default();
    activity.begin("Updating roadmap");
    activity.observe(&tool("a", ToolStatus::InProgress));
    activity.observe(&tool("b", ToolStatus::InProgress));
    activity.observe(&tool("a", ToolStatus::Completed));
    assert_eq!(activity.status, "Reading");
    activity.observe(&tool("b", ToolStatus::Failed));
    assert_eq!(activity.status, "Waiting for response");
    assert!(activity.active, "tool failure is not a response boundary");
}

#[test]
fn late_text_and_tool_updates_cannot_undo_stopping() {
    let mut activity = AgentActivity::default();
    activity.begin("Conversation");
    activity.observe(&tool("a", ToolStatus::InProgress));
    activity.stopping();
    activity.observe(&AgentEvent::Text("last output".to_owned()));
    assert_eq!(activity.status, "Stopping");
    activity.observe(&tool("a", ToolStatus::Completed));
    assert_eq!(activity.status, "Stopping");
    activity.observe(&AgentEvent::TurnEnded(TurnEnd::Cancelled));
    assert_eq!(activity.status, "Stopped");
    assert!(!activity.active);
}

#[test]
fn thinking_is_never_inferred_from_submitting_or_running_a_reasoning_tool() {
    let mut activity = AgentActivity::default();
    activity.begin("Conversation");
    assert_eq!(activity.status, "Waiting for response");
    let mut event = tool("reasoning", ToolStatus::InProgress);
    if let AgentEvent::ToolCall { verb, .. } = &mut event {
        *verb = ToolVerb::Think;
    }
    activity.observe(&event);
    assert_eq!(activity.status, "Recording reasoning");
    activity.observe(&tool("reasoning", ToolStatus::Completed));
    activity.observe(&AgentEvent::Thought("A recorded thought".to_owned()));
    assert_eq!(activity.status, "Thinking");
}
