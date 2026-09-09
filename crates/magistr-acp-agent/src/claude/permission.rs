//! `can_use_tool` ↔ ACP `session/request_permission`.

use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionKind, RequestPermissionOutcome, ToolCallUpdate,
    ToolCallUpdateFields,
};
use serde::Deserialize;
use serde_json::Value;

use crate::claude::tool_kind::tool_kind;

/// A permission ask the CLI sent, and both halves of its relay: the ACP
/// request it becomes, and the control response the client's answer becomes.
///
/// The CLI asks after it has announced the `tool_use` block, so the ask
/// names a tool call the client already holds; the ACP request is a
/// `ToolCallUpdate` on that id whose title is the tool's **raw name**
/// (`Bash`, `Write`, `mcp__magistr__roadmap_status`) and whose kind is the
/// name's ACP kind — a client's permission policy keys on exactly those
/// (allowed-tool sets, auto-allow rules, risk classification, auto-approval
/// by kind), and a display name in their place turns every known tool into
/// an unknown one that only a human can answer. The client answers with one
/// of three options:
///
/// | option id | kind | control response |
/// |---|---|---|
/// | `allow` | allow once | `{behavior: allow, updatedInput}` |
/// | `allow_always` | allow always | the same plus `updatedPermissions`: the CLI's own suggestions (a session-scoped rule or mode) |
/// | `reject` | reject once | `{behavior: deny, message}` — the tool result comes back as an error |
///
/// A cancelled ask (the client cancelled the turn) is a deny that also asks
/// the CLI to interrupt.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct PermissionAsk {
    /// The tool's name.
    pub tool_name: String,
    /// The tool's display name, when the CLI gives one.
    pub display_name: String,
    /// The CLI's one-line description of this particular call.
    pub description: String,
    /// The tool input, echoed back unchanged on allow.
    pub input: Value,
    /// What "always allow" would persist — the CLI's suggestion, passed
    /// through verbatim.
    pub permission_suggestions: Vec<Value>,
    /// The `tool_use` block's id — the ACP tool call id.
    pub tool_use_id: String,
}

impl Default for PermissionAsk {
    fn default() -> Self {
        Self {
            tool_name: String::new(),
            display_name: String::new(),
            description: String::new(),
            input: Value::Null,
            permission_suggestions: Vec::new(),
            tool_use_id: String::new(),
        }
    }
}

impl PermissionAsk {
    /// The `allow` option's id.
    pub const ALLOW: &'static str = "allow";
    /// The `allow_always` option's id.
    pub const ALLOW_ALWAYS: &'static str = "allow_always";
    /// The `reject` option's id.
    pub const REJECT: &'static str = "reject";

    /// Read a `can_use_tool` request. `None` when it names no tool call.
    pub fn parse(request: &Value) -> Option<Self> {
        let ask: Self = serde_json::from_value(request.clone()).ok()?;
        (!ask.tool_use_id.is_empty()).then_some(ask)
    }

    /// The tool call the ask names, as the permission request carries it:
    /// the raw tool name as the title, the name's kind, and the input.
    pub fn tool_call(&self) -> ToolCallUpdate {
        ToolCallUpdate::new(
            self.tool_use_id.clone(),
            ToolCallUpdateFields::new()
                .title(self.tool_name.clone())
                .kind(tool_kind(&self.tool_name))
                .raw_input(self.input.clone()),
        )
    }

    /// The options the client chooses from.
    pub fn options(&self) -> Vec<PermissionOption> {
        vec![
            PermissionOption::new(Self::ALLOW, "Allow", PermissionOptionKind::AllowOnce),
            PermissionOption::new(
                Self::ALLOW_ALWAYS,
                "Allow for this session",
                PermissionOptionKind::AllowAlways,
            ),
            PermissionOption::new(Self::REJECT, "Reject", PermissionOptionKind::RejectOnce),
        ]
    }

    /// The control response payload for the client's answer.
    pub fn answer(&self, outcome: &RequestPermissionOutcome) -> Value {
        match outcome {
            RequestPermissionOutcome::Selected(selected) => {
                match selected.option_id.to_string().as_str() {
                    Self::ALLOW => serde_json::json!({
                        "behavior": "allow",
                        "updatedInput": self.input,
                    }),
                    Self::ALLOW_ALWAYS => serde_json::json!({
                        "behavior": "allow",
                        "updatedInput": self.input,
                        "updatedPermissions": self.permission_suggestions,
                    }),
                    _ => serde_json::json!({
                        "behavior": "deny",
                        "message": "The user rejected this tool call",
                    }),
                }
            }
            _ => serde_json::json!({
                "behavior": "deny",
                "message": "The turn was cancelled",
                "interrupt": true,
            }),
        }
    }
}

#[cfg(test)]
#[path = "permission.tests.rs"]
mod tests;
