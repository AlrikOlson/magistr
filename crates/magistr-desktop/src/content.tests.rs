use super::*;
use magistr::application::RetainedObservation;

#[test]
fn authored_text_is_exact_and_internal_events_are_not_copy() {
    let source = "# 日本\r\n<command> &amp; **café**\n";
    let record = RetainedObservation {
        id: 2,
        sequence: 9,
        message: Msg::Agent(AgentEvent::Text(source.into())),
    };
    let value = observation(&record).unwrap();
    assert_eq!(value.text, source);
    assert_eq!((value.id, value.revision), (2, 9));
    assert!(value.document.is_some());
    assert!(
        observation(&RetainedObservation {
            message: Msg::Agent(AgentEvent::Terminal {
                terminal_id: "t".into(),
                bytes: vec![27]
            }),
            ..record
        })
        .is_none()
    );
}

#[test]
fn accepted_files_are_visible_without_replacing_authored_text_or_copying_bytes() {
    use magistr_application::sessions::{PromptAttachment, PromptInput};
    let source = "  日本\r\n<command> &amp;  ";
    let input = PromptInput {
        text: source.into(),
        attachments: vec![
            PromptAttachment::from_bytes("notes.txt", "hash", b"private file contents".to_vec())
                .unwrap(),
        ],
    };
    let value = observation(&RetainedObservation {
        id: 3,
        sequence: 4,
        message: Msg::InputAccepted(input.receipt()),
    })
    .unwrap();
    assert_eq!(value.role, "You");
    assert_eq!(value.text, source);
    assert_eq!(value.attachments[0].id, input.attachments[0].id());
    assert_eq!(value.attachments[0].name, "notes.txt");
    assert_eq!(value.attachments[0].source, "Text file attached");
    assert!(
        !serde_json::to_string(&value)
            .unwrap()
            .contains("private file contents")
    );
    let files_only = observation(&RetainedObservation {
        id: 5,
        sequence: 6,
        message: Msg::InputAccepted(
            PromptInput {
                text: String::new(),
                ..input
            }
            .receipt(),
        ),
    })
    .unwrap();
    assert!(files_only.text.is_empty());
    assert_eq!(files_only.attachments.len(), 1);
}

#[test]
fn tool_projection_keeps_exact_details_error_status_and_truncation() {
    let input = "日本\r\n  input  ";
    let record = RetainedObservation {
        id: 4,
        sequence: 20,
        message: Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "call".into(),
            title: Some("Build".into()),
            status: Some(ToolStatus::Failed),
            diff: None,
            duration_ms: Some(25),
            images: Vec::new(),
            record: Box::new(ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Text(
                    input.into(),
                )),
                text: vec!["full error\n".into()],
                truncated_from: Some(100000),
                ..ToolRecord::default()
            }),
        }),
    };
    let value = observation(&record).unwrap();
    assert!(value.text.contains(input));
    assert_eq!(value.tool.as_ref().unwrap().details[0].source, input);
    assert_eq!(value.tool.as_ref().unwrap().status, "Failed");
    assert_eq!(value.tool.unwrap().truncated_from, Some(100000));
}
