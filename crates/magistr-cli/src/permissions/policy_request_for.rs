//! Build a governance [`PolicyRequest`] from a tool call.

use magistr_engine::policy::{PolicyRequest, RiskTier};

use super::classify_risk::classify_risk;
use super::risk_level::RiskLevel;

/// Describe a tool call to the governance policy engine.
///
/// Gathers the fields a [`PolicyCondition`] can ask about: the tool name,
/// the shell command when there is one, every path the call names, the
/// session's model, and this project's own risk classification mapped
/// onto the engine's tier.
///
/// [`PolicyCondition`]: magistr_engine::policy::PolicyCondition
#[must_use]
pub fn policy_request_for(
    tool_name: &str,
    tool_input: &serde_json::Value,
    model: Option<&str>,
) -> PolicyRequest {
    let mut request =
        PolicyRequest::tool(tool_name).with_risk(match classify_risk(tool_name, tool_input) {
            RiskLevel::Low => RiskTier::Low,
            RiskLevel::Medium => RiskTier::Medium,
            RiskLevel::High => RiskTier::High,
        });

    if let Some(command) = tool_input.get("command").and_then(|v| v.as_str()) {
        request = request.with_command(command);
    }

    for field in [
        "file_path",
        "path",
        "notebook_path",
        "target_file",
        "source",
        "destination",
        "source_path",
        "destination_path",
        "old_path",
        "new_path",
        "from",
        "to",
    ] {
        if let Some(path) = tool_input.get(field).and_then(|v| v.as_str()) {
            request = request.with_path(path);
        }
    }
    if let Some(paths) = tool_input.get("paths").and_then(|v| v.as_array()) {
        for path in paths.iter().filter_map(|v| v.as_str()) {
            request = request.with_path(path);
        }
    }

    if let Some(model) = model {
        request = request.with_model(model);
    }

    request
}

#[cfg(test)]
#[path = "policy_request_for.tests.rs"]
mod tests;
