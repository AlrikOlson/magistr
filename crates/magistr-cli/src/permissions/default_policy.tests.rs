use super::*;

use crate::permissions::permission_profile::PermissionProfile;
use crate::permissions::policy::PermissionPolicy;
use crate::permissions::profile_action::ProfileAction;

#[test]
fn default_policy_allows_read_on_standard_profile() {
    let policy = DefaultPermissionPolicy::new(PermissionProfile::Standard, Vec::new(), Vec::new());
    let decision = policy.evaluate("Read", &serde_json::json!({"file_path": "/src/main.rs"}));
    assert_eq!(decision.action, ProfileAction::Allow);
}

#[test]
fn default_policy_asks_for_write_on_standard_profile() {
    let policy = DefaultPermissionPolicy::new(PermissionProfile::Standard, Vec::new(), Vec::new());
    let decision = policy.evaluate("Write", &serde_json::json!({"file_path": "/src/new.rs"}));
    assert_eq!(decision.action, ProfileAction::Ask);
}

#[test]
fn default_policy_usable_as_trait_object() {
    let policy: Box<dyn PermissionPolicy> = Box::new(DefaultPermissionPolicy::new(
        PermissionProfile::Standard,
        Vec::new(),
        Vec::new(),
    ));
    let decision = policy.evaluate("Read", &serde_json::json!({}));
    assert_eq!(decision.action, ProfileAction::Allow);
}
