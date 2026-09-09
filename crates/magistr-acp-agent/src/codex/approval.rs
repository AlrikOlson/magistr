//! Codex command/file approvals are delegated to the ACP client's policy.

use crate::AgentError;
use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionKind, RequestPermissionOutcome, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind,
};
use serde_json::{Value, json};

pub(crate) struct Approval {
    call: ToolCallUpdate,
    allow_once: bool,
    allow_session: bool,
}

#[cfg(test)]
#[path = "approval.tests.rs"]
mod tests;

impl Approval {
    pub fn parse(method: &str, params: &Value, item: Option<&Value>) -> Result<Self, AgentError> {
        let id = params["itemId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| AgentError::provider("malformed Codex approval: missing itemId"))?;
        let command = method == "item/commandExecution/requestApproval";
        let (title, kind) = if command {
            ("Bash", ToolKind::Execute)
        } else {
            ("Edit", ToolKind::Edit)
        };
        let mut input = params.clone();
        if let Some(item) = item {
            input["item"] = item.clone();
        }
        if !command {
            input["paths"] = json!(patch_paths(item)?);
        }
        let allow_session = !command
            || params["availableDecisions"]
                .as_array()
                .is_none_or(|decisions| {
                    decisions
                        .iter()
                        .any(|decision| decision == "acceptForSession")
                });
        let allow_once = !command
            || params["availableDecisions"]
                .as_array()
                .is_none_or(|decisions| decisions.iter().any(|decision| decision == "accept"));
        Ok(Self {
            call: ToolCallUpdate::new(
                id.to_owned(),
                ToolCallUpdateFields::new()
                    .title(title)
                    .kind(kind)
                    .raw_input(input),
            ),
            allow_once,
            allow_session,
        })
    }

    pub fn tool_call(&self) -> ToolCallUpdate {
        self.call.clone()
    }

    pub fn options(&self) -> Vec<PermissionOption> {
        let mut options = Vec::new();
        if self.allow_once {
            options.push(PermissionOption::new(
                "allow",
                "Allow",
                PermissionOptionKind::AllowOnce,
            ));
        }
        if self.allow_session {
            options.push(PermissionOption::new(
                "allow_always",
                "Allow for this session",
                PermissionOptionKind::AllowAlways,
            ));
        }
        options.push(PermissionOption::new(
            "reject",
            "Reject",
            PermissionOptionKind::RejectOnce,
        ));
        options
    }

    pub fn answer(&self, outcome: &RequestPermissionOutcome) -> Value {
        let decision = match outcome {
            RequestPermissionOutcome::Selected(selected) => {
                match selected.option_id.to_string().as_str() {
                    "allow" if self.allow_once => "accept",
                    "allow_always" if self.allow_session => "acceptForSession",
                    _ => "decline",
                }
            }
            _ => "cancel",
        };
        json!({"decision":decision})
    }
}

/// Expose every write and rename target through the ACP policy's common
/// path field, while keeping the runtime's full patch under `item`.
fn patch_paths(item: Option<&Value>) -> Result<Vec<&str>, AgentError> {
    let item = item
        .filter(|item| item["type"] == "fileChange")
        .ok_or_else(|| {
            AgentError::provider("Codex file approval has no matching fileChange item")
        })?;
    let changes = item["changes"]
        .as_array()
        .filter(|changes| !changes.is_empty())
        .ok_or_else(|| AgentError::provider("Codex file approval has no complete patch scope"))?;
    let mut paths = Vec::new();
    for change in changes {
        let path = change["path"]
            .as_str()
            .filter(|path| !path.is_empty())
            .ok_or_else(|| AgentError::provider("Codex file approval has a missing patch path"))?;
        if !paths.contains(&path) {
            paths.push(path);
        }
        if let Some(destination) = change["kind"]
            .get("move_path")
            .filter(|path| !path.is_null())
        {
            let destination = destination
                .as_str()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    AgentError::provider("Codex file approval has an invalid move destination")
                })?;
            if !paths.contains(&destination) {
                paths.push(destination);
            }
        }
    }
    Ok(paths)
}
