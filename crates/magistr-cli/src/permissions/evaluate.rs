//! Evaluate whether a tool call should be allowed, denied, or prompted.

use super::auto_allow_rule::AutoAllowRule;
use super::classify_risk::classify_risk;
use super::decision_reason::DecisionReason;
use super::permission_decision::PermissionDecision;
use super::permission_profile::PermissionProfile;
use super::profile_action::ProfileAction;

/// Evaluate whether a tool call should be allowed, denied, or prompted.
///
/// Checks in order:
/// 1. Explicit auto-allow rules (from config)
/// 2. Batch approval patterns (quality gate commands)
/// 3. Profile default for the risk level
pub fn evaluate(
    tool_name: &str,
    tool_input: &serde_json::Value,
    profile: PermissionProfile,
    auto_rules: &[AutoAllowRule],
    gate_commands: &[String],
) -> PermissionDecision {
    let risk = classify_risk(tool_name, tool_input);

    // Check auto-allow rules first
    for rule in auto_rules {
        if rule.matches(tool_name, tool_input) {
            return PermissionDecision {
                tool_name: tool_name.to_string(),
                risk,
                action: ProfileAction::Allow,
                reason: DecisionReason::AutoAllowRule(rule.description.clone()),
            };
        }
    }

    // Check batch approval for quality gate commands
    if tool_name == "Bash"
        && let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str())
    {
        for gate_cmd in gate_commands {
            if cmd.trim() == gate_cmd.trim() || cmd.contains(gate_cmd.as_str()) {
                return PermissionDecision {
                    tool_name: tool_name.to_string(),
                    risk,
                    action: ProfileAction::Allow,
                    reason: DecisionReason::BatchApproval(gate_cmd.clone()),
                };
            }
        }
    }

    // Fall back to profile default
    let action = profile.default_action(risk);
    PermissionDecision {
        tool_name: tool_name.to_string(),
        risk,
        action,
        reason: DecisionReason::ProfileDefault,
    }
}
