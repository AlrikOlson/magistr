use super::*;
use magistr_application::sessions::{ToolPayload, ToolRecord, ToolStatus, ToolVerb};

#[test]
fn tool_revisions_keep_original_identity_and_complete_fields() {
    let mut record = RetainedObservation::new(
        3,
        Msg::Agent(AgentEvent::ToolCall {
            id: "call".into(),
            title: "Read café".into(),
            verb: ToolVerb::Read,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                input: Some(ToolPayload::Text("original\r\n日本".into())),
                ..ToolRecord::default()
            }),
        }),
    );
    record.update_tool(
        19,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "call".into(),
            title: None,
            status: Some(ToolStatus::Failed),
            diff: None,
            duration_ms: Some(40),
            images: Vec::new(),
            record: Box::new(ToolRecord {
                text: vec!["error details".into()],
                ..ToolRecord::default()
            }),
        }),
    );
    assert_eq!((record.id, record.sequence), (3, 19));
    let Msg::Agent(AgentEvent::ToolCallUpdate {
        title,
        status,
        record,
        duration_ms,
        ..
    }) = record.message
    else {
        panic!("tool");
    };
    assert_eq!(title.as_deref(), Some("Read café"));
    assert_eq!(status, Some(ToolStatus::Failed));
    assert_eq!(duration_ms, Some(40));
    assert_eq!(record.input.unwrap().content(), "original\r\n日本");
    assert_eq!(record.text, ["error details"]);
}

#[test]
fn full_stream_revisions_preserve_exact_source_and_first_identity() {
    let mut record = RetainedObservation::new(2, Msg::Agent(AgentEvent::Text("# 日本\r\n".into())));
    assert!(record.append(5, &Msg::Agent(AgentEvent::Text("**café**\n".into()))));
    assert!(!record.append(6, &Msg::Agent(AgentEvent::Thought("different".into()))));
    assert_eq!((record.id, record.sequence), (2, 5));
    assert_eq!(
        record.message,
        Msg::Agent(AgentEvent::Text("# 日本\r\n**café**\n".into()))
    );
}
