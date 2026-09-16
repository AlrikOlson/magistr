//! Fictional conversation shared by public captures and production visual checks.
use magistr_cockpit::agent::{AgentEvent, EditDiff, ToolRecord, ToolStatus, ToolVerb};
use magistr_cockpit::cockpit::{Model, Msg, update};
use serde_json::{Value, json};
use std::error::Error;

/// Feed ordinary session events through the production update path.
pub fn conversation(model: &mut Model, data: &Value) -> Result<(), Box<dyn Error>> {
    let text = |key: &str| -> Result<String, Box<dyn Error>> {
        data[key]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("missing Harbor conversation field: {key}").into())
    };
    for event in [
        AgentEvent::Initialized {
            session_id: "harbor-demo".into(),
            model: None,
        },
        AgentEvent::UserText(text("prompt")?),
        AgentEvent::Text(text("response")?),
        AgentEvent::ToolCall {
            id: "harbor-edit".into(),
            title: "Save the retry schedule".into(),
            verb: ToolVerb::Edit,
            status: ToolStatus::Completed,
            diff: Some(EditDiff {
                path: text("edit_path")?,
                old_text: text("old_text")?,
                new_text: text("new_text")?,
            }),
            images: Vec::new(),
            record: Box::new(ToolRecord {
                text: vec![text("edit_summary")?],
                locations: vec![(text("edit_path")?.into(), None)],
                ..ToolRecord::default()
            }),
        },
        AgentEvent::ToolCall {
            id: "harbor-test".into(),
            title: "3 recovery checks passed".into(),
            verb: ToolVerb::Run,
            status: ToolStatus::Completed,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                input: Some(json!({"command": text("command")?})),
                output: Some(json!({"passed": 3, "failed": 0})),
                terminal: Some("harbor-test-terminal".into()),
                ..ToolRecord::default()
            }),
        },
        AgentEvent::Terminal {
            terminal_id: "harbor-test-terminal".into(),
            bytes: text("terminal")?.into_bytes(),
        },
        AgentEvent::Text(text("followup")?),
    ] {
        update(model, Msg::Agent(event));
    }
    Ok(())
}
