//! Source-preserving desktop messages and complete tool details are one projection
//! of the retained application records; formatting never replaces their source.
use crate::document::Document;
use magistr_application::sessions::{AgentEvent, EditDiff, ToolRecord, ToolStatus};
use magistr_cockpit::cockpit::Msg;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Observation {
    pub(crate) id: u64,
    pub(crate) revision: u64,
    pub(crate) role: &'static str,
    pub(crate) text: String,
    pub(crate) document: Option<Document>,
    pub(crate) tool: Option<Tool>,
    pub(crate) attachments: Vec<SentAttachment>,
}
#[derive(Serialize)]
pub(crate) struct SentAttachment {
    id: String,
    name: String,
    size: usize,
    source: &'static str,
}
#[derive(Serialize)]
pub(crate) struct Detail {
    pub(crate) label: &'static str,
    pub(crate) source: String,
}
#[derive(Serialize)]
pub(crate) struct Tool {
    pub(crate) title: String,
    pub(crate) status: &'static str,
    pub(crate) details: Vec<Detail>,
    pub(crate) truncated_from: Option<usize>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) image_count: usize,
}

pub(crate) fn observation(
    record: &magistr::application::RetainedObservation,
) -> Option<Observation> {
    let mut result = Observation {
        id: record.id,
        revision: record.sequence,
        role: "",
        text: String::new(),
        document: None,
        tool: None,
        attachments: Vec::new(),
    };
    let (role, source, rich) = match &record.message {
        Msg::InputAccepted(input) => {
            result.attachments = input
                .attachments
                .iter()
                .map(|file| SentAttachment {
                    id: file.id.clone(),
                    name: file.name.clone(),
                    size: file.byte_len,
                    source: match file.kind {
                        magistr_application::sessions::PromptAttachmentKind::Image => {
                            "Image attached"
                        }
                        magistr_application::sessions::PromptAttachmentKind::Text => {
                            "Text file attached"
                        }
                    },
                })
                .collect();
            ("You", input.text.clone(), false)
        }
        Msg::CommandFailed { error, .. } => ("Error", error.clone(), false),
        Msg::Agent(event) => match event {
            AgentEvent::Text(text) => ("Agent", text.clone(), true),
            AgentEvent::Thought(text) => ("Thinking", text.clone(), true),
            AgentEvent::UserText(text) => ("You", text.clone(), false),
            AgentEvent::Notice(text) => ("Magistr", text.clone(), false),
            AgentEvent::Error(text) | AgentEvent::PromptRejected { error: text, .. } => {
                ("Error", text.clone(), false)
            }
            AgentEvent::PermissionLapsed { title, .. } => (
                "Magistr",
                format!("The request expired and was denied: {title}"),
                false,
            ),
            AgentEvent::Lagged { .. } => (
                "Magistr",
                "Some earlier updates are unavailable.".into(),
                false,
            ),
            AgentEvent::ToolCallUpdate {
                title,
                status,
                record,
                diff,
                duration_ms,
                images,
                ..
            } => {
                let tool = tool(
                    title.as_deref().unwrap_or("Command"),
                    *status,
                    record,
                    diff.as_ref(),
                    *duration_ms,
                    images.len(),
                );
                let mut source = format!("{}\nStatus: {}", tool.title, tool.status);
                for detail in &tool.details {
                    source.push_str(&format!("\n\n{}\n{}", detail.label, detail.source));
                }
                if let Some(bytes) = tool.truncated_from {
                    source.push_str(&format!(
                        "\n\nSome tool details are unavailable. Original size: {bytes} bytes."
                    ));
                }
                result.tool = Some(tool);
                ("Command", source, false)
            }
            _ => return None,
        },
        _ => return None,
    };
    result.role = role;
    result.document = rich.then(|| Document::parse(&source));
    result.text = source;
    Some(result)
}

fn tool(
    title: &str,
    status: Option<ToolStatus>,
    record: &ToolRecord,
    diff: Option<&EditDiff>,
    duration_ms: Option<u64>,
    image_count: usize,
) -> Tool {
    let mut details = Vec::new();
    if let Some(input) = &record.input {
        details.push(Detail {
            label: "Input",
            source: input.content().into(),
        });
    }
    if let Some(output) = &record.output {
        details.push(Detail {
            label: "Output",
            source: output.content().into(),
        });
    }
    for text in &record.text {
        details.push(Detail {
            label: "Result",
            source: text.clone(),
        });
    }
    if let Some(diff) = diff {
        details.push(Detail {
            label: "File",
            source: diff.path.clone(),
        });
        details.push(Detail {
            label: "Before",
            source: diff.old_text.clone(),
        });
        details.push(Detail {
            label: "After",
            source: diff.new_text.clone(),
        });
    }
    for (path, line) in &record.locations {
        details.push(Detail {
            label: "Location",
            source: line.map_or_else(|| path.clone(), |line| format!("{path}:{line}")),
        });
    }
    Tool {
        title: title.into(),
        status: match status {
            Some(ToolStatus::Pending) => "Waiting",
            Some(ToolStatus::InProgress) => "Running",
            Some(ToolStatus::Completed) => "Finished",
            Some(ToolStatus::Failed) => "Failed",
            None => "Updated",
        },
        details,
        truncated_from: record.truncated_from,
        duration_ms,
        image_count,
    }
}

#[cfg(test)]
#[path = "content.tests.rs"]
mod tests;
