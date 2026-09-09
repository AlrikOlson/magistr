use super::*;

use std::sync::Mutex;

use crate::permissions::decision_reason::DecisionReason;
use crate::permissions::permission_decision::PermissionDecision;
use crate::permissions::profile_action::ProfileAction;
use crate::permissions::risk_level::RiskLevel;

/// Mock permission policy that returns preconfigured decisions.
pub struct MockPermissionPolicy {
    action: ProfileAction,
    risk: RiskLevel,
    /// Records tool names that were evaluated (for assertion).
    evaluated: Mutex<Vec<String>>,
}

impl MockPermissionPolicy {
    /// Create a mock that always allows.
    pub fn allow_all() -> Self {
        Self {
            action: ProfileAction::Allow,
            risk: RiskLevel::Low,
            evaluated: Mutex::new(Vec::new()),
        }
    }

    /// Create a mock that always asks.
    pub fn ask_all() -> Self {
        Self {
            action: ProfileAction::Ask,
            risk: RiskLevel::Medium,
            evaluated: Mutex::new(Vec::new()),
        }
    }

    /// Get the list of tool names that were evaluated.
    pub fn evaluated_tools(&self) -> Vec<String> {
        self.evaluated
            .lock()
            .expect("MockPermissionPolicy lock poisoned")
            .clone()
    }
}

impl PermissionPolicy for MockPermissionPolicy {
    fn evaluate(&self, tool_name: &str, _tool_input: &serde_json::Value) -> PermissionDecision {
        self.evaluated
            .lock()
            .expect("MockPermissionPolicy lock poisoned")
            .push(tool_name.to_string());

        PermissionDecision {
            tool_name: tool_name.to_string(),
            risk: self.risk,
            action: self.action,
            reason: DecisionReason::ProfileDefault,
        }
    }
}

#[test]
fn mock_policy_allows_everything() {
    let policy = MockPermissionPolicy::allow_all();
    let decision = policy.evaluate("Bash", &serde_json::json!({"command": "ls"}));
    assert_eq!(decision.action, ProfileAction::Allow);
    assert_eq!(decision.tool_name, "Bash");
}

#[test]
fn mock_policy_asks_for_everything() {
    let policy = MockPermissionPolicy::ask_all();
    let decision = policy.evaluate("Write", &serde_json::json!({"path": "/tmp/foo"}));
    assert_eq!(decision.action, ProfileAction::Ask);
}

#[test]
fn mock_policy_records_evaluations() {
    let policy = MockPermissionPolicy::allow_all();
    policy.evaluate("Read", &serde_json::json!({}));
    policy.evaluate("Write", &serde_json::json!({}));
    policy.evaluate("Bash", &serde_json::json!({}));

    let evaluated = policy.evaluated_tools();
    assert_eq!(evaluated, vec!["Read", "Write", "Bash"]);
}

#[test]
fn mock_policy_usable_as_trait_object() {
    let policy: Box<dyn PermissionPolicy> = Box::new(MockPermissionPolicy::allow_all());
    let decision = policy.evaluate("Read", &serde_json::json!({}));
    assert_eq!(decision.action, ProfileAction::Allow);
}
